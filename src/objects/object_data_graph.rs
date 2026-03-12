use std::{ffi::c_void, ops::ControlFlow};

use indexmap::IndexMap;

use crate::{
    elf::symbol::{Symbol, SymbolBinding, SymbolVisibility},
    error::MirosError,
    objects::object_data::ObjectData,
};

pub struct ObjectDataGraph {
    pub(crate) program: ObjectData,
    pub(crate) miros: ObjectData,
    pub(crate) dependencies: IndexMap<String, ObjectData>,
}

impl ObjectDataGraph {
    pub fn new(program: ObjectData, miros: ObjectData) -> Self {
        Self {
            program,
            miros,
            dependencies: IndexMap::new(),
        }
    }

    pub fn iter_objects(&self) -> impl DoubleEndedIterator<Item = &ObjectData> {
        std::iter::once(&self.program).chain(self.dependencies.values())
    }

    pub fn iter_objects_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut ObjectData> {
        std::iter::once(&mut self.program).chain(self.dependencies.values_mut())
    }

    // DFS post-order topological sort — dependencies before dependents, cycles skipped
    pub fn iter_objects_topological(&self) -> impl DoubleEndedIterator<Item = &ObjectData> {
        enum Event<'a> {
            Discover(usize),
            Emit(&'a ObjectData),
        }

        let objects: Vec<&ObjectData> = self.dependencies.values().collect();
        let mut visited = vec![false; objects.len()];
        let mut order: Vec<&ObjectData> = Vec::with_capacity(objects.len() + 1);

        let mut stack: Vec<Event> = (0..objects.len()).rev().map(Event::Discover).collect();

        while let Some(event) = stack.pop() {
            match event {
                Event::Discover(index) => {
                    if visited[index] {
                        continue;
                    }
                    visited[index] = true;

                    let object = objects[index];
                    stack.push(Event::Emit(object));

                    for needed in object.dynamic_fields.dependencies() {
                        if let Some(needed_index) = self.dependencies.get_index_of(*needed) {
                            if !visited[needed_index] {
                                stack.push(Event::Discover(needed_index));
                            }
                        }
                    }
                }
                Event::Emit(object) => order.push(object),
            }
        }

        order.push(&self.program);
        order.into_iter()
    }

    pub fn resolve_symbol_address(
        &self,
        symbol: Symbol,
        requesting_object: &ObjectData,
    ) -> Result<*const c_void, MirosError> {
        let symbol_name = unsafe {
            requesting_object
                .dynamic_fields
                .string_table
                .get(symbol.st_name as usize)
        };

        // NOTE: Protected symbols cannot be interposed - bind to the requesting object's own definition.
        let protected_symbol = requesting_object
            .resolve_symbol_and_address(symbol_name)
            .filter(|(symbol, _)| {
                symbol.st_other.symbol_visibility() == SymbolVisibility::Protected
            })
            .map(|(_, address)| address);

        if let Some(address) = protected_symbol {
            return Ok(address);
        }

        let resolved = self
            .iter_objects()
            .chain(std::iter::once(&self.miros))
            .flat_map(|object| object.resolve_symbol_and_address(symbol_name))
            .try_fold(None, |first_weak, (symbol, address)| {
                match symbol.binding() {
                    Ok(SymbolBinding::Global) => ControlFlow::Break(address),
                    Ok(SymbolBinding::Weak) => ControlFlow::Continue(first_weak.or(Some(address))),
                    _ => ControlFlow::Continue(first_weak),
                }
            });

        match resolved {
            ControlFlow::Break(address) => Ok(address),
            ControlFlow::Continue(Some(address)) => Ok(address),
            ControlFlow::Continue(None) => {
                Err(MirosError::UndefinedSymbol(symbol_name.to_string()))
            }
        }
    }
}
