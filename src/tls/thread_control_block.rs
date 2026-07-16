use std::{
    ffi::c_void,
    mem::offset_of,
    sync::atomic::{AtomicU32, Ordering},
};

use strum::FromRepr;

use crate::utils::mremap_allocator::MreMapAllocator;

const _: () = assert!(offset_of!(ThreadControlBlock, canary) == 0x28);

#[repr(C)]
pub struct ThreadControlBlock {
    pub thread_pointee: [u8; 0],
    pub thread_pointer_register: *mut c_void,
    pub tid: i32,
    pub detach_state: AtomicDetachState,
    pub return_value: *mut c_void,
    pub region: *mut [u8],
    pub canary: usize,
    pub dynamic_thread_vector: DynamicThreadVector,
}

/// The detach/reap handshake between `pthread_detach` and the exiting thread; see `libc::threads::self_detach`.
#[derive(FromRepr, PartialEq, Clone, Copy)]
#[repr(u32)]
pub enum DetachState {
    Joinable = 0,
    Detached = 1,
    Exiting = 2,
}

/// `AtomicU32`-backed cell for `DetachState`; the field values only ever come from the enum, so every read round-trips through `from_repr` unwrapped.
/// Orderings are fixed to the handshake's one use.
pub struct AtomicDetachState(AtomicU32);

impl AtomicDetachState {
    pub const fn new(state: DetachState) -> Self {
        Self(AtomicU32::new(state as u32))
    }

    pub fn swap(&self, state: DetachState) -> DetachState {
        DetachState::from_repr(self.0.swap(state as u32, Ordering::AcqRel)).unwrap()
    }

    pub fn compare_exchange(
        &self,
        current: DetachState,
        new: DetachState,
    ) -> Result<DetachState, DetachState> {
        self.0
            .compare_exchange(
                current as u32,
                new as u32,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(|value| DetachState::from_repr(value).unwrap())
            .map_err(|value| DetachState::from_repr(value).unwrap())
    }
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
