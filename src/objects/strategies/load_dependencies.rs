use std::collections::VecDeque;

use crate::{
    error::MirosError,
    objects::{object_data::ObjectData, object_data_graph::ObjectDataGraph, strategies::Stratagem},
};

const INTERCEPTED_LIBRARIES: &[&str] = &["libc.so.6", "libpthread.so.0", "ld-linux-x86-64.so.2"];

pub struct LoadDependencies {}

impl LoadDependencies {
    pub fn new() -> Self {
        Self {}
    }
}

impl Stratagem for LoadDependencies {
    fn run(&self, object_data: &mut ObjectDataGraph) -> Result<(), MirosError> {
        let mut pending: VecDeque<(String, Option<String>)> = object_data
            .program
            .dynamic_fields
            .dependencies()
            .iter()
            .map(|name| (name.to_string(), None))
            .collect();

        while let Some((dependency_name, declarer_key)) = pending.pop_front() {
            if object_data.dependencies.contains_key(&dependency_name)
                || INTERCEPTED_LIBRARIES.contains(&dependency_name.as_str())
            {
                continue;
            }

            let path_resolver = match &declarer_key {
                None => &object_data.program.dynamic_fields.path_resolver,
                Some(key) => &object_data.dependencies[key].dynamic_fields.path_resolver,
            };

            let file = path_resolver.resolve(&dependency_name)?;
            let loaded_object = unsafe { ObjectData::from_file(file)? };

            let transitive_dependencies: Vec<(String, Option<String>)> = loaded_object
                .dynamic_fields
                .dependencies()
                .iter()
                .map(|name| (name.to_string(), Some(dependency_name.clone())))
                .collect();

            object_data
                .dependencies
                .insert(dependency_name, loaded_object);

            pending.extend(transitive_dependencies);
        }

        Ok(())
    }
}
