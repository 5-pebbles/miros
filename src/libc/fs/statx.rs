use core::ffi::{c_char, c_int, c_uint};

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn statx(
    directory_fd: c_int,
    pathname: *const c_char,
    flags: c_int,
    mask: c_uint,
    status: *mut libc::statx,
) -> c_int {
    signature_matches_libc!(libc::statx(directory_fd, pathname, flags, mask, status));
    let result = syscall!(Syscall::Statx, directory_fd, pathname, flags, mask, status);
    if result < 0 {
        set_errno(Errno(result.abs() as u32));
        -1
    } else {
        0
    }
}
