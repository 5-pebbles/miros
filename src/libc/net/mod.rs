use std::ffi::c_int;

use crate::libc::errno::{set_errno, Errno};

// This is the ABI with the kernel; glibc's `struct sockaddr` is a 16-byte fixed field.
#[allow(non_camel_case_types)]
pub(crate) type sockaddr = linux_raw_sys::net::__kernel_sockaddr_storage;
#[allow(non_camel_case_types)]
pub(crate) type socklen_t = u32;

// NOTE: SOCK_CLOEXEC/SOCK_NONBLOCK live in include/linux/net.h, not the uapi headers bindgen reads.
pub(crate) const SOCK_CLOEXEC: c_int = 0o2000000;
pub(crate) const SOCK_NONBLOCK: c_int = 0o4000;

mod accept;
mod connect;
mod listen;
mod names;
mod socket;

// Arguments forward to the syscall in declaration order; a negative result maps to errno + -1.
macro_rules! net_syscall_pass_through {
    (fn $name:ident($($argument:ident: $argument_type:ty),* $(,)?) = $syscall:ident) => {
        #[cfg_attr(not(test), no_mangle)]
        pub(crate) unsafe extern "C" fn $name($($argument: $argument_type),*) -> std::ffi::c_int {
            $crate::signature_matches_libc!(libc::$name($(std::mem::transmute($argument)),*));

            $crate::libc::net::translate_syscall_result($crate::syscall!(
                $crate::syscall::Syscall::$syscall,
                $($argument),*
            ))
        }
    };
}

pub(crate) use net_syscall_pass_through;

/// The kernel reports errors as -errno; the C ABI reports them through the thread-local errno.
pub(crate) fn translate_syscall_result(result: isize) -> c_int {
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        result as c_int
    }
}
