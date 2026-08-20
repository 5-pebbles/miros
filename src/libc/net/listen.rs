use std::ffi::c_int;

use crate::{
    libc::net::{sockaddr, socklen_t, translate_syscall_result},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
pub(crate) unsafe extern "C" fn bind(
    socket: c_int,
    address: *const sockaddr,
    address_len: socklen_t,
) -> c_int {
    signature_matches_libc!(libc::bind(
        socket,
        std::mem::transmute(address),
        std::mem::transmute(address_len),
    ));

    translate_syscall_result(syscall!(Syscall::Bind, socket, address, address_len))
}

#[cfg_attr(not(test), no_mangle)]
pub(crate) unsafe extern "C" fn listen(socket: c_int, backlog: c_int) -> c_int {
    signature_matches_libc!(libc::listen(socket, backlog));

    translate_syscall_result(syscall!(Syscall::Listen, socket, backlog))
}
