use core::ffi::c_int;

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn lseek64(file_descriptor: c_int, offset: i64, whence: c_int) -> i64 {
    signature_matches_libc!(libc::lseek64(file_descriptor, offset, whence));
    let result = syscall!(Syscall::LSeek, file_descriptor, offset, whence);
    if result < 0 {
        set_errno(Errno(result.abs() as u32));
        -1
    } else {
        result as i64
    }
}
