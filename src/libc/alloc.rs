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

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn posix_memalign(
    memory_pointer: *mut *mut c_void,
    alignment: usize,
    size: usize,
) -> i32 {
    signature_matches_libc!(libc::posix_memalign(memory_pointer, alignment, size));

    // POSIX: a power of two & a multiple of the pointer width, else EINVAL with `*memptr` untouched.
    if !alignment.is_power_of_two() || alignment % size_of::<*const c_void>() != 0 {
        return Errno::INVAL.0 as i32;
    }

    match primary().alloc(Layout::from_size_align_unchecked(size, alignment)) {
        Some(pointer) => {
            *memory_pointer = pointer.as_ptr().cast();
            0
        }
        None => Errno::NOMEM.0 as i32,
    }
}

/// Shared core for the return-a-pointer aligned allocators: null with EINVAL on a bad alignment.
#[inline(always)]
unsafe fn aligned_allocation(alignment: usize, size: usize) -> *mut c_void {
    // `Layout` requires a power-of-two alignment; a non-power-of-two would be UB below.
    if !alignment.is_power_of_two() {
        set_errno(Errno::INVAL);
        return ptr::null_mut();
    }
    allocation_or_nomem(primary().alloc(Layout::from_size_align_unchecked(size, alignment)))
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn aligned_alloc(alignment: usize, size: usize) -> *mut c_void {
    signature_matches_libc!(libc::aligned_alloc(alignment, size));
    aligned_allocation(alignment, size)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn memalign(alignment: usize, size: usize) -> *mut c_void {
    signature_matches_libc!(libc::memalign(alignment, size));
    aligned_allocation(alignment, size)
}
