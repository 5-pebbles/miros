use std::collections::VecDeque;

use crate::{
    error::MirosError,
    objects::{
        object_data::{Dynamic, ObjectData},
        object_data_map::ObjectDataMap,
        strategies::Stratagem,
    },
};

pub struct LoadDependencies {}

impl LoadDependencies {
    pub fn new() -> Self {
        Self {}
    }
}

impl Stratagem<ObjectDataMap> for LoadDependencies {
    fn run(&self, object_data: &mut ObjectDataMap) -> Result<(), MirosError> {
        let mut pending: VecDeque<(String, Option<String>)> = object_data
            .program
            .dynamic_fields
            .dependencies()
            .map(|name| (name.to_string(), None))
            .collect();

        while let Some((dependency_name, declarer_key)) = pending.pop_front() {
            if object_data.dependencies.contains_key(&dependency_name) {
                continue;
            }

            let path_resolver = match &declarer_key {
                None => &object_data.program.dynamic_fields.path_resolver,
                Some(key) => &object_data.dependencies[key].dynamic_fields.path_resolver,
            };

            let file = path_resolver.resolve(&dependency_name)?;
            let loaded_object = unsafe { ObjectData::<Dynamic>::from_file(file)? };

            let transitive_dependencies: Vec<(String, Option<String>)> = loaded_object
                .dynamic_fields
                .dependencies()
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
