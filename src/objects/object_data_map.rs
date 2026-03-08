use std::{
    collections::HashMap,
    hash::{BuildHasherDefault, DefaultHasher, Hash, Hasher},
};

use crate::objects::object_data::{Dynamic, ObjectData};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LibraryNameHash(u64);

impl LibraryNameHash {
    pub fn new(name: &str) -> Self {
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        Self(hasher.finish())
    }
}

impl Hash for LibraryNameHash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.0);
    }
}

#[derive(Default)]
pub struct IdentityHasher(u64);

impl Hasher for IdentityHasher {
    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }

    // Fallback for non-u64 hashing — not used by LibraryNameHash.
    fn write(&mut self, bytes: &[u8]) {
        self.0 = bytes
            .iter()
            .take(8)
            .fold(0, |hash, byte| hash << 8 | *byte as u64);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

type IdentityBuildHasher = BuildHasherDefault<IdentityHasher>;

pub struct ObjectDataMap {
    pub(crate) program: ObjectData<Dynamic>,
    pub(crate) dependencies: HashMap<LibraryNameHash, ObjectData<Dynamic>, IdentityBuildHasher>,
}

impl ObjectDataMap {
    pub fn new(program: ObjectData<Dynamic>) -> Self {
        Self {
            program,
            dependencies: HashMap::with_hasher(IdentityBuildHasher::default()),
        }
    }
}
