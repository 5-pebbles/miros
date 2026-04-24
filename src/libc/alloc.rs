use std::{alloc::Layout, ffi::c_void, ptr};

use crate::{allocator::PRIMARY, signature_matches_libc};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    signature_matches_libc!(libc::malloc(size));
    #[allow(static_mut_refs)]
    let allocator = PRIMARY.assume_init_mut();
    allocator.alloc(Layout::from_size_align_unchecked(size, 1)) as *mut c_void
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut c_void {
    signature_matches_libc!(libc::calloc(count, size));
    let total = match count.checked_mul(size) {
        Some(total) => total,
        None => return ptr::null_mut(),
    };
    #[allow(static_mut_refs)]
    let allocator = PRIMARY.assume_init_mut();
    allocator.alloc_zeroed(Layout::from_size_align_unchecked(total, 1)) as *mut c_void
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
    signature_matches_libc!(libc::realloc(pointer, size));
    #[allow(static_mut_refs)]
    let allocator = PRIMARY.assume_init_mut();
    allocator.realloc(pointer as *mut u8, size) as *mut c_void
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn free(pointer: *mut c_void) {
    signature_matches_libc!(libc::free(pointer));
    #[allow(static_mut_refs)]
    let allocator = PRIMARY.assume_init_mut();
    allocator.free(pointer as *mut u8);
}
