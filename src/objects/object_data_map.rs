use std::collections::HashMap;

use crate::objects::object_data::{Dynamic, ObjectData};

pub struct ObjectDataMap {
    pub(crate) program: ObjectData<Dynamic>,
    pub(crate) dependencies: HashMap<String, ObjectData<Dynamic>>,
}

impl ObjectDataMap {
    pub fn new(program: ObjectData<Dynamic>) -> Self {
        Self {
            program,
            dependencies: HashMap::new(),
        }
    }
}
