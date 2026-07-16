use core::ffi::c_int;

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn socketpair(
    domain: c_int,
    socket_type: c_int,
    protocol: c_int,
    socket_vector: *mut c_int,
) -> c_int {
    signature_matches_libc!(libc::socketpair(
        domain,
        socket_type,
        protocol,
        socket_vector
    ));
    let result = syscall!(
        Syscall::SocketPair,
        domain,
        socket_type,
        protocol,
        socket_vector
    );
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}
