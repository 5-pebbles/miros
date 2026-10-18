use core::ffi::c_int;

use crate::{signature_matches_libc, syscall, syscall::Syscall};

// Unlike the syscall, POSIX `posix_fadvise` returns the error number directly and leaves errno alone.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn posix_fadvise(
    file_descriptor: c_int,
    offset: i64,
    length: i64,
    advice: c_int,
) -> c_int {
    signature_matches_libc!(libc::posix_fadvise(file_descriptor, offset, length, advice));

    let result = syscall!(Syscall::FAdvise64, file_descriptor, offset, length, advice);
    if result < 0 {
        (-result) as c_int
    } else {
        0
    }
}
