use core::ffi::c_int;

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn poll(
    file_descriptors: *mut libc::pollfd,
    count: libc::nfds_t,
    timeout: c_int,
) -> c_int {
    signature_matches_libc!(libc::poll(file_descriptors, count, timeout));
    let result = syscall!(Syscall::Poll, file_descriptors, count, timeout);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        result as c_int
    }
}
