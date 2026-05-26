use std::{ffi::c_void, mem::offset_of};

use crate::utils::mremap_allocator::MreMapAllocator;

const _: () = assert!(offset_of!(ThreadControlBlock, canary) == 0x28);

#[repr(C)]
pub struct ThreadControlBlock {
    pub thread_pointee: [u8; 0],
    pub thread_pointer_register: *mut c_void,
    pub tid: i32,
    pub _padding: [u8; 4],
    pub return_value: *mut c_void,
    pub region: *mut [u8],
    pub canary: usize,
    pub dynamic_thread_vector: DynamicThreadVector,
}

pub struct DynamicThreadVector {
    last_updated_generation: usize,
    values: Vec<DynamicThreadVectorItem, MreMapAllocator>,
}

impl DynamicThreadVector {
    pub fn new() -> Self {
        Self {
            last_updated_generation: 0,
            values: Vec::new_in(MreMapAllocator),
        }
    }
}

pub struct DynamicThreadVectorItem {
    last_updated_generation: usize,
    block_pointer: *mut u8,
}
