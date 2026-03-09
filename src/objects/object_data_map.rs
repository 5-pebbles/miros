use std::collections::HashMap;

use crate::objects::object_data::ObjectData;

pub struct ObjectDataMap {
    pub(crate) program: ObjectData,
    pub(crate) dependencies: HashMap<String, ObjectData>,
}

impl ObjectDataMap {
    pub fn new(program: ObjectData) -> Self {
        Self {
            program,
            dependencies: HashMap::new(),
        }
    }

    pub fn iter_objects(&self) -> impl Iterator<Item = &ObjectData> {
        std::iter::once(&self.program).chain(self.dependencies.values())
    }

    pub fn iter_objects_mut(&mut self) -> impl Iterator<Item = &mut ObjectData> {
        std::iter::once(&mut self.program).chain(self.dependencies.values_mut())
    }
}
