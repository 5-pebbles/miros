use std::{ffi::c_void, sync::OnceLock};

use crate::{
    error::Result,
    objects::{object_data::ObjectData, object_data_graph::ObjectDataGraph, strategies::Stratagem},
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

static STARTUP_INIT_ARRAY: OnceLock<InitArray> = OnceLock::new();

// SAFETY: these pointers reference immutable process-lifetime startup data.
unsafe impl Send for InitArray {}
unsafe impl Sync for InitArray {}

impl InitArray {
    pub unsafe fn install(
        arg_count: usize,
        arg_pointer: *const *const u8,
        env_pointer: *const *const u8,
        auxv_pointer: *const AuxiliaryVectorItem,
    ) -> &'static Self {
        STARTUP_INIT_ARRAY.get_or_init(|| InitArray {
            arg_count,
            arg_pointer,
            env_pointer,
            auxv_pointer,
        })
    }

    pub unsafe fn get() -> &'static Self {
        STARTUP_INIT_ARRAY
            .get()
            .expect("startup arguments are not installed")
    }

    pub fn run_preinit(&self, object: &ObjectData) {
        if let Some(preinit_functions) = object.dynamic_fields.preinit_functions() {
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
    }

    pub fn run_init(&self, object: &ObjectData) {
        if let Some(init_function) = object.dynamic_fields.init_function() {
            init_function(
                self.arg_count,
                self.arg_pointer,
                self.env_pointer,
                self.auxv_pointer,
            );
        }
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
    }
}

impl Stratagem for InitArray {
    fn run(&self, object_data: &mut ObjectDataGraph) -> Result<()> {
        self.run_preinit(&object_data.program);
        object_data.iter_objects_topological().for_each(|object| {
            self.run_init(object);
        });

        Ok(())
    }
}

pub type FiniArrayFunction = extern "C" fn();

pub fn run_fini_array(object: &ObjectData) {
    if let Some(fini_functions) = object.dynamic_fields.fini_functions() {
        #[allow(useless_ptr_null_checks)]
        fini_functions
            .iter()
            .rev()
            .filter(|fini_fn| !(**fini_fn as *const c_void).is_null())
            .for_each(|fini_fn| fini_fn());
    }
    if let Some(fini_function) = object.dynamic_fields.fini_function() {
        fini_function();
    }
}
