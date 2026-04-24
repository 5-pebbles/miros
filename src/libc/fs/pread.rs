use std::{
    ffi::c_void,
    os::fd::{AsRawFd, BorrowedFd},
};

use crate::{
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pread64(
    file_descriptor: BorrowedFd<'_>,
    buffer_pointer: *mut c_void,
    buffer_length_in_bytes: usize,
    offset: i64,
) -> isize {
    signature_matches_libc!(libc::pread64(
        std::mem::transmute(file_descriptor),
        buffer_pointer.cast(),
        buffer_length_in_bytes,
        offset
    ));

    syscall!(
        Syscall::PRead64,
        file_descriptor.as_raw_fd(),
        buffer_pointer,
        buffer_length_in_bytes,
        offset
    )
}
