use std::{mem::MaybeUninit, sync::Mutex};

use crate::{
    objects::strategies::init_array::InitArrayFunction, tls::layout_allocator::TlsLayoutAllocator,
};

mod layout_allocator;
mod module_registry;
pub mod template;
pub mod thread_control_block;

pub const TLS_RESERVE_SIZE: usize = 8 * 1024 * 1024;

pub static mut TLS_RESERVE_ALLOCATOR: MaybeUninit<Mutex<TlsLayoutAllocator>> =
    MaybeUninit::uninit();

#[cfg_attr(not(test), link_section = ".init_array")]
#[used]
static INIT_TLS_RESERVE_ALLOCATOR: InitArrayFunction = init_tls_reserve_allocator;

extern "C" fn init_tls_reserve_allocator(
    _arg_count: usize,
    _arg_pointer: *const *const u8,
    _env_pointer: *const *const u8,
    _auxv_pointer: *const crate::start::auxiliary_vector::AuxiliaryVectorItem,
) {
    unsafe {
        #[allow(static_mut_refs)]
        TLS_RESERVE_ALLOCATOR.write(Mutex::new(TlsLayoutAllocator::new()));
    }
}

pub fn get_tls_reserve_allocator_ref() -> &'static Mutex<TlsLayoutAllocator> {
    #[allow(static_mut_refs)]
    unsafe {
        TLS_RESERVE_ALLOCATOR.assume_init_ref()
    }
}
