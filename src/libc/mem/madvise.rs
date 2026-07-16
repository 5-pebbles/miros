use core::ffi::{c_int, c_void};

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn madvise(address: *mut c_void, length: libc::size_t, advice: c_int) -> c_int {
    signature_matches_libc!(libc::madvise(address, length, advice));
    let result = syscall!(Syscall::MAdvise, address, length, advice);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}
