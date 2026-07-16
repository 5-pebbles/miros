use core::ffi::c_char;
use std::ptr;

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn getcwd(buffer: *mut c_char, size: libc::size_t) -> *mut c_char {
    signature_matches_libc!(libc::getcwd(buffer, size));
    let result = syscall!(Syscall::GetCwd, buffer, size);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        ptr::null_mut()
    } else {
        buffer
    }
}
