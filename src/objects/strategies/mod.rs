use crate::{
    error::MirosError,
    objects::object_data::{AnyDynamic, Dynamic, NonDynamic, ObjectData},
};

pub mod init_array;
pub mod load_dependencies;
pub mod relocate;
pub mod thread_local_storage;

pub type ObjectDataVector = Vec<ObjectData<Dynamic>>;
pub type ObjectDataSingle = ObjectData<NonDynamic>;

pub trait AsObjectDataSlice {
    type Item: AnyDynamic;
    fn as_object_slice_mut(&mut self) -> &mut [ObjectData<Self::Item>];
}

impl AsObjectDataSlice for ObjectData<NonDynamic> {
    type Item = NonDynamic;
    fn as_object_slice_mut(&mut self) -> &mut [ObjectData<NonDynamic>] {
        std::slice::from_mut(self)
    }
}

impl AsObjectDataSlice for Vec<ObjectData<Dynamic>> {
    type Item = Dynamic;
    fn as_object_slice_mut(&mut self) -> &mut [ObjectData<Dynamic>] {
        self.as_mut_slice()
    }
}

pub trait Stratagem<T> {
    fn run(&self, object_data: &mut T) -> Result<(), MirosError>;
}
