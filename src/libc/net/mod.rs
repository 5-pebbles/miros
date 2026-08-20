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
mod epoll;
mod eventfd;
mod listen;
mod names;
mod options;
mod poll;
mod select;
mod socket;
mod transfer;

// A C function per entry: `Syscall::X` forwards the arguments in declaration order, `{ body }`
// is written out in full. Both map a negative result to errno + -1 via `translate`.
macro_rules! net_syscall_pass_through {
    () => {};
    (fn $name:ident($($argument:ident: $argument_type:ty),* $(,)?) -> $return_type:ty = Syscall::$syscall:ident; $($rest:tt)*) => {
        #[cfg_attr(not(test), no_mangle)]
        pub(crate) unsafe extern "C" fn $name($($argument: $argument_type),*) -> $return_type {
            $crate::signature_matches_libc!(libc::$name($(std::mem::transmute($argument)),*));

            let result = $crate::syscall!($crate::syscall::Syscall::$syscall, $($argument),*);
            $crate::libc::net::translate(result)
        }
        net_syscall_pass_through! { $($rest)* }
    };
    (fn $name:ident($($argument:ident: $argument_type:ty),* $(,)?) -> $return_type:ty { $($body:tt)* } $($rest:tt)*) => {
        #[cfg_attr(not(test), no_mangle)]
        pub(crate) unsafe extern "C" fn $name($($argument: $argument_type),*) -> $return_type {
            $($body)*
        }
        net_syscall_pass_through! { $($rest)* }
    };
}

pub(crate) trait TranslateSyscallResult {
    fn translate(result: isize) -> Self;
}

impl TranslateSyscallResult for std::ffi::c_int {
    fn translate(result: isize) -> Self {
        translate_syscall_result(result)
    }
}

impl TranslateSyscallResult for isize {
    fn translate(result: isize) -> Self {
        if result < 0 {
            crate::libc::errno::set_errno(crate::libc::errno::Errno(result.unsigned_abs() as u32));
            -1
        } else {
            result
        }
    }
}

pub(crate) fn translate<ReturnType: TranslateSyscallResult>(result: isize) -> ReturnType {
    TranslateSyscallResult::translate(result)
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
