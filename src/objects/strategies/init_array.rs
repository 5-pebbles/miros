use std::ffi::c_void;

use crate::{
    error::MirosError,
    objects::{object_data_graph::ObjectDataGraph, strategies::Stratagem},
    start::auxiliary_vector::AuxiliaryVectorItem,
};

pub type InitArrayFunction =
    extern "C" fn(usize, *const *const u8, *const *const u8, *const AuxiliaryVectorItem);

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

impl Stratagem for InitArray {
    fn run(&self, object_data: &mut ObjectDataGraph) -> Result<(), MirosError> {
        if let Some(preinit_functions) = object_data.program.dynamic_fields.preinit_functions() {
            // SAFETY: The compiler thinks function pointers can't be null in Rust's type system,
            // but these are unsafely read from raw ELF init_array data...
            #[allow(useless_ptr_null_checks)]
            preinit_functions
                .iter()
                .filter(|preinit_fn| !(**preinit_fn as *const c_void).is_null())
                .for_each(|preinit_fn| {
                    preinit_fn(
                        self.arg_count,
                        self.arg_pointer,
                        self.env_pointer,
                        self.auxv_pointer,
                    )
                });
        }

        object_data.iter_objects_topological().for_each(|object| {
            if let Some(init_functions) = object.dynamic_fields.init_functions() {
                #[allow(useless_ptr_null_checks)]
                init_functions
                    .iter()
                    .filter(|init_fn| !(**init_fn as *const c_void).is_null())
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
