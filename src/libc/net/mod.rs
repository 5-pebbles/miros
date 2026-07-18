use core::ffi::{c_int, c_void};

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn socket(domain: c_int, socket_type: c_int, protocol: c_int) -> c_int {
    signature_matches_libc!(libc::socket(domain, socket_type, protocol));
    let result = syscall!(Syscall::Socket, domain, socket_type, protocol);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        result as c_int
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn socketpair(
    domain: c_int,
    socket_type: c_int,
    protocol: c_int,
    socket_vector: *mut c_int,
) -> c_int {
    signature_matches_libc!(libc::socketpair(domain, socket_type, protocol, socket_vector));
    let result = syscall!(Syscall::SocketPair, domain, socket_type, protocol, socket_vector);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn bind(
    socket: c_int,
    address: *const libc::sockaddr,
    address_length: libc::socklen_t,
) -> c_int {
    signature_matches_libc!(libc::bind(socket, address, address_length));
    let result = syscall!(Syscall::Bind, socket, address, address_length);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn listen(socket: c_int, backlog: c_int) -> c_int {
    signature_matches_libc!(libc::listen(socket, backlog));
    let result = syscall!(Syscall::Listen, socket, backlog);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn accept4(
    socket: c_int,
    address: *mut libc::sockaddr,
    address_length: *mut libc::socklen_t,
    flags: c_int,
) -> c_int {
    signature_matches_libc!(libc::accept4(socket, address, address_length, flags));
    let result = syscall!(Syscall::Accept4, socket, address, address_length, flags);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        result as c_int
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn setsockopt(
    socket: c_int,
    level: c_int,
    option_name: c_int,
    option_value: *const c_void,
    option_length: libc::socklen_t,
) -> c_int {
    signature_matches_libc!(libc::setsockopt(
        socket,
        level,
        option_name,
        option_value,
        option_length
    ));
    let result = syscall!(
        Syscall::SetSockOpt,
        socket,
        level,
        option_name,
        option_value,
        option_length
    );
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn getsockopt(
    socket: c_int,
    level: c_int,
    option_name: c_int,
    option_value: *mut c_void,
    option_length: *mut libc::socklen_t,
) -> c_int {
    signature_matches_libc!(libc::getsockopt(
        socket,
        level,
        option_name,
        option_value,
        option_length
    ));
    let result = syscall!(
        Syscall::GetSockOpt,
        socket,
        level,
        option_name,
        option_value,
        option_length
    );
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn getsockname(
    socket: c_int,
    address: *mut libc::sockaddr,
    address_length: *mut libc::socklen_t,
) -> c_int {
    signature_matches_libc!(libc::getsockname(socket, address, address_length));
    let result = syscall!(Syscall::GetSockName, socket, address, address_length);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn getpeername(
    socket: c_int,
    address: *mut libc::sockaddr,
    address_length: *mut libc::socklen_t,
) -> c_int {
    signature_matches_libc!(libc::getpeername(socket, address, address_length));
    let result = syscall!(Syscall::GetPeerName, socket, address, address_length);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn recv(
    socket: c_int,
    buffer: *mut c_void,
    length: usize,
    flags: c_int,
) -> isize {
    signature_matches_libc!(libc::recv(socket, buffer, length, flags));
    let result = syscall!(Syscall::RecvFrom, socket, buffer, length, flags, 0usize, 0usize);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        result
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn send(
    socket: c_int,
    buffer: *const c_void,
    length: usize,
    flags: c_int,
) -> isize {
    signature_matches_libc!(libc::send(socket, buffer, length, flags));
    let result = syscall!(Syscall::SendTo, socket, buffer, length, flags, 0usize, 0usize);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        result
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn shutdown(socket: c_int, how: c_int) -> c_int {
    signature_matches_libc!(libc::shutdown(socket, how));
    let result = syscall!(Syscall::Shutdown, socket, how);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}
