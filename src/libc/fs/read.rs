use std::{
    ffi::c_void,
    os::fd::{AsRawFd, BorrowedFd},
};

use crate::{libc::translate_syscall_result, signature_matches_libc, syscall, syscall::Syscall};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn read(
    file_descriptor: BorrowedFd<'_>,
    buffer_pointer: *mut c_void,
    buffer_length_in_bytes: usize,
) -> isize {
    signature_matches_libc!(libc::read(
        std::mem::transmute(file_descriptor),
        buffer_pointer.cast(),
        buffer_length_in_bytes
    ));

    let result = syscall!(
        Syscall::Read,
        file_descriptor.as_raw_fd(),
        buffer_pointer,
        buffer_length_in_bytes
    );
    translate_syscall_result(result)
}
