use std::{ffi::c_void, ptr::null};

use crate::{
    error::MirosError,
    objects::strategies::{ObjectDataCollection, Stratagem},
    start::auxiliary_vector::AuxiliaryVectorItem,
};

pub struct InitArray {
    arg_count: usize,
    arg_pointer: *const *const u8,
    env_pointer: *const *const u8,
    auxv_pointer: *const AuxiliaryVectorItem,
}

impl InitArray {
    pub unsafe fn new(
        arg_count: usize,
        arg_pointer: *const *const u8,
        env_pointer: *const *const u8,
        auxv_pointer: *const AuxiliaryVectorItem,
    ) -> Self {
        Self {
            arg_count,
            arg_pointer,
            env_pointer,
            auxv_pointer,
        }
    }
}

impl<T: ObjectDataCollection> Stratagem<T> for InitArray {
    fn run(&self, object_data: &mut T) -> Result<(), MirosError> {
        object_data.iter_objects().for_each(|object| {
            if let Some(init_functions) = unsafe { object.dynamic_fields.init_functions() } {
                init_functions
                    .iter()
                    .filter(|init_fn| **init_fn as *const c_void != null())
                    .for_each(|init_fn| {
                        init_fn(
                            self.arg_count,
                            self.arg_pointer,
                            self.env_pointer,
                            self.auxv_pointer,
                        )
                    });
            }
        });
        Ok(())
    }
}
