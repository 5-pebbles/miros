use std::os::fd::{AsRawFd, BorrowedFd};

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

const TCGETS: usize = 0x5401;

/// A successful terminal-attributes fetch is the definition of a tty; any error means it isn't one.
pub fn file_descriptor_isatty(file_descriptor: BorrowedFd<'_>) -> bool {
    let mut terminal_attributes = [0u8; 64];
    let result = unsafe {
        syscall!(
            Syscall::IoCtl,
            file_descriptor.as_raw_fd(),
            TCGETS,
            terminal_attributes.as_mut_ptr()
        )
    };
    result == 0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn isatty(file_descriptor: BorrowedFd<'_>) -> i32 {
    signature_matches_libc!(libc::isatty(file_descriptor.as_raw_fd()));

    if file_descriptor_isatty(file_descriptor) {
        1
    } else {
        set_errno(Errno(linux_raw_sys::errno::ENOTTY));
        0
    }
}
