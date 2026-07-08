use std::{alloc::Layout, ffi::c_void, ptr, ptr::NonNull};

use crate::{
    allocator::primary,
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
};

/// Translate the allocator's `Option` result to the C ABI: a failed allocation is null with `ENOMEM`.
#[inline(always)]
fn allocation_or_nomem(result: Option<NonNull<u8>>) -> *mut c_void {
    match result {
        Some(pointer) => pointer.as_ptr().cast(),
        None => {
            set_errno(Errno::NOMEM);
            ptr::null_mut()
        }
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    signature_matches_libc!(libc::malloc(size));
    allocation_or_nomem(primary().alloc(Layout::from_size_align_unchecked(size, 1)))
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut c_void {
    signature_matches_libc!(libc::calloc(count, size));
    let total = match count.checked_mul(size) {
        Some(total) => total,
        None => {
            set_errno(Errno::NOMEM);
            return ptr::null_mut();
        }
    };
    allocation_or_nomem(primary().alloc_zeroed(Layout::from_size_align_unchecked(total, 1)))
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
    signature_matches_libc!(libc::realloc(pointer, size));
    match primary().realloc(pointer as *mut u8, size) {
        Some(new_pointer) => new_pointer.as_ptr().cast(),
        // `realloc(p, 0)` frees and returns null by design; only a non-zero request that fails is ENOMEM.
        None => {
            if size != 0 {
                set_errno(Errno::NOMEM);
            }
            ptr::null_mut()
        }
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn free(pointer: *mut c_void) {
    signature_matches_libc!(libc::free(pointer));
    primary().free(pointer as *mut u8)
}
