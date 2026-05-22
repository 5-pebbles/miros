use std::ffi::c_void;

use crate::{
    error::MirosError,
    objects::{object_data::ObjectData, object_data_graph::ObjectDataGraph, strategies::Stratagem},
    syscall::thread_pointer::get_thread_pointer,
};

pub struct ThreadLocalStorage {
    pseudorandom_bytes: *const [u8; 16],
}

impl ThreadLocalStorage {
    pub fn new(pseudorandom_bytes: *const [u8; 16]) -> Self {
        Self { pseudorandom_bytes }
    }
}

impl ThreadLocalStorage {
    fn allocate_tls_module(&self, object: &ObjectData, thread_pointer: *const c_void) {
        todo!()
    }
}

impl Stratagem for ThreadLocalStorage {
    fn run(&self, object_data: &mut ObjectDataGraph) -> Result<(), MirosError> {
        let thread_pointer = unsafe { get_thread_pointer() };
        object_data
            .iter_objects()
            .for_each(|object| self.allocate_tls_module(object, thread_pointer));
        Ok(())
    }
}
