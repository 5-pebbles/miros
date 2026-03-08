use std::{collections::HashMap, hash::Hasher};

use crate::objects::object_data::{Dynamic, ObjectData};

pub struct IdentityHasher(u64);

impl Hasher for IdentityHasher {
    fn write(&mut self, bytes: &[u8]) {
        self.0 = bytes
            .into_iter()
            .take(8)
            .fold(0, |hash, byte| hash << 8 | *byte as u64);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

pub struct ObjectDataMap {
    pub(crate) program: ObjectData<Dynamic>,
    pub(crate) dependencies: HashMap<u64, ObjectData<Dynamic>>,
}

impl ObjectDataMap {
    pub fn new(program: ObjectData<Dynamic>) -> Self {
        Self {
            program,
            dependencies: HashMap::new(),
        }
    }
}
