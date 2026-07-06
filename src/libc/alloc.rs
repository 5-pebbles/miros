use std::{alloc::Layout, ffi::c_void, ptr};

use crate::{allocator::primary, signature_matches_libc};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    signature_matches_libc!(libc::malloc(size));
    primary().alloc(Layout::from_size_align_unchecked(size, 1)) as *mut c_void
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut c_void {
    signature_matches_libc!(libc::calloc(count, size));
    let total = match count.checked_mul(size) {
        Some(total) => total,
        None => return ptr::null_mut(),
    };
    primary().alloc_zeroed(Layout::from_size_align_unchecked(total, 1)) as *mut c_void
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
    signature_matches_libc!(libc::realloc(pointer, size));
    primary().realloc(pointer as *mut u8, size) as *mut c_void
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn free(pointer: *mut c_void) {
    signature_matches_libc!(libc::free(pointer));
    primary().free(pointer as *mut u8)
}
