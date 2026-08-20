use std::ffi::c_int;

use crate::libc::errno::{set_errno, Errno};

// NOTE: SOCK_CLOEXEC/SOCK_NONBLOCK live in include/linux/net.h, not the uapi headers bindgen reads.
pub(crate) const SOCK_CLOEXEC: c_int = 0o2000000;
pub(crate) const SOCK_NONBLOCK: c_int = 0o4000;

mod socket;

/// The kernel reports errors as -errno; the C ABI reports them through the thread-local errno.
pub(crate) fn translate_syscall_result(result: isize) -> c_int {
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        result as c_int
    }
}
