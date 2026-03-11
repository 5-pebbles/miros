use std::{ffi::c_void, ops::ControlFlow};

use indexmap::IndexMap;

use crate::{
    elf::symbol::{Symbol, SymbolBinding, SymbolVisibility},
    error::MirosError,
    objects::object_data::ObjectData,
};

pub struct ObjectDataMap {
    pub(crate) program: ObjectData,
    pub(crate) miros: ObjectData,
    pub(crate) dependencies: IndexMap<String, ObjectData>,
}

impl ObjectDataMap {
    pub fn new(program: ObjectData, miros: ObjectData) -> Self {
        Self {
            program,
            miros,
            dependencies: IndexMap::new(),
        }
    }

    pub fn iter_objects(&self) -> impl Iterator<Item = &ObjectData> {
        std::iter::once(&self.program)
            .chain(self.dependencies.values())
            .chain(std::iter::once(&self.miros))
    }

    pub fn iter_objects_mut(&mut self) -> impl Iterator<Item = &mut ObjectData> {
        std::iter::once(&mut self.program)
            .chain(self.dependencies.values_mut())
            .chain(std::iter::once(&mut self.miros))
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
