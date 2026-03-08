use crate::{
    error::MirosError,
    objects::{
        object_data::{AnyDynamic, Dynamic, NonDynamic, ObjectData},
        object_data_map::ObjectDataMap,
    },
};

pub mod init_array;
pub mod load_dependencies;
pub mod relocate;
pub mod thread_local_storage;

pub trait ObjectDataCollection {
    type Item: AnyDynamic;
    fn iter_objects(&self) -> impl Iterator<Item = &ObjectData<Self::Item>>;
}

impl ObjectDataCollection for ObjectData<NonDynamic> {
    type Item = NonDynamic;
    fn iter_objects(&self) -> impl Iterator<Item = &ObjectData<NonDynamic>> {
        std::iter::once(self)
    }
}

impl ObjectDataCollection for ObjectDataMap {
    type Item = Dynamic;
    fn iter_objects(&self) -> impl Iterator<Item = &ObjectData<Dynamic>> {
        std::iter::once(&self.program).chain(self.dependencies.values())
    }
}

pub trait Stratagem<T> {
    fn run(&self, object_data: &mut T) -> Result<(), MirosError>;
}
