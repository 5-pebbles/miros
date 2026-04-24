use std::os::fd::RawFd;

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn close(file_descriptor: RawFd) -> i32 {
    signature_matches_libc!(libc::close(file_descriptor));

    if file_descriptor == -1 {
        set_errno(Errno::BADF);
        return -1;
    }

    let result = syscall!(Syscall::Close, file_descriptor);
    result as i32
}
