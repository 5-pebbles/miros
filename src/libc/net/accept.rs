use std::ffi::c_int;

use crate::{
    libc::net::{sockaddr, socklen_t, translate_syscall_result},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
pub(crate) unsafe extern "C" fn accept(
    socket: c_int,
    address: *mut sockaddr,
    address_len: *mut socklen_t,
) -> c_int {
    signature_matches_libc!(libc::accept(
        socket,
        std::mem::transmute(address),
        std::mem::transmute(address_len),
    ));

    translate_syscall_result(syscall!(Syscall::Accept, socket, address, address_len))
}

#[cfg_attr(not(test), no_mangle)]
pub(crate) unsafe extern "C" fn accept4(
    socket: c_int,
    address: *mut sockaddr,
    address_len: *mut socklen_t,
    flags: c_int,
) -> c_int {
    signature_matches_libc!(libc::accept4(
        socket,
        std::mem::transmute(address),
        std::mem::transmute(address_len),
        flags,
    ));

    translate_syscall_result(syscall!(
        Syscall::Accept4,
        socket,
        address,
        address_len,
        flags
    ))
}
