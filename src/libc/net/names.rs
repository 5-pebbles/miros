use std::ffi::c_int;

use crate::{
    libc::net::{sockaddr, socklen_t, translate_syscall_result},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
pub(crate) unsafe extern "C" fn getsockname(
    socket: c_int,
    address: *mut sockaddr,
    address_len: *mut socklen_t,
) -> c_int {
    signature_matches_libc!(libc::getsockname(
        socket,
        std::mem::transmute(address),
        std::mem::transmute(address_len),
    ));

    translate_syscall_result(syscall!(Syscall::GetSockName, socket, address, address_len))
}

#[cfg_attr(not(test), no_mangle)]
pub(crate) unsafe extern "C" fn getpeername(
    socket: c_int,
    address: *mut sockaddr,
    address_len: *mut socklen_t,
) -> c_int {
    signature_matches_libc!(libc::getpeername(
        socket,
        std::mem::transmute(address),
        std::mem::transmute(address_len),
    ));

    translate_syscall_result(syscall!(Syscall::GetPeerName, socket, address, address_len))
}
