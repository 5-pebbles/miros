use core::ffi::c_int;
use std::os::fd::{AsRawFd, BorrowedFd};

use crate::{libc::translate_syscall_result, signature_matches_libc, syscall, syscall::Syscall};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn lseek64(file_descriptor: BorrowedFd<'_>, offset: i64, whence: c_int) -> i64 {
    signature_matches_libc!(libc::lseek64(
        std::mem::transmute(file_descriptor),
        offset,
        whence
    ));

    let result = syscall!(Syscall::LSeek, file_descriptor.as_raw_fd(), offset, whence);
    translate_syscall_result(result) as i64
}
