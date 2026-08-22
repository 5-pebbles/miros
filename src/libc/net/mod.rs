use std::ffi::c_int;

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
// is written out in full. The `Syscall::X` form maps a negative result to errno + -1.
macro_rules! net_syscall_pass_through {
    () => {};
    (fn $name:ident($($argument:ident: $argument_type:ty),* $(,)?) -> $return_type:ty = Syscall::$syscall:ident; $($rest:tt)*) => {
        #[cfg_attr(not(test), no_mangle)]
        pub(crate) unsafe extern "C" fn $name($($argument: $argument_type),*) -> $return_type {
            $crate::signature_matches_libc!(libc::$name($(std::mem::transmute($argument)),*));

            let result = $crate::syscall!($crate::syscall::Syscall::$syscall, $($argument),*);
            $crate::libc::translate_syscall_result(result) as $return_type
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

pub(crate) use net_syscall_pass_through;
