use core::ffi::{c_int, c_uint};

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

// glibc's `eventfd` is the two-argument `eventfd2` syscall; the one-argument original is never used.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn eventfd(initial_value: c_uint, flags: c_int) -> c_int {
    signature_matches_libc!(libc::eventfd(initial_value, flags));
    let result = syscall!(Syscall::Eventfd2, initial_value, flags);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        result as c_int
    }
}
