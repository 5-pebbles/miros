use core::ffi::c_int;

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn writev(
    file_descriptor: c_int,
    io_vectors: *const libc::iovec,
    count: c_int,
) -> isize {
    signature_matches_libc!(libc::writev(file_descriptor, io_vectors, count));
    let result = syscall!(Syscall::WriteV, file_descriptor, io_vectors, count);
    if result < 0 {
        set_errno(Errno(result.abs() as u32));
        -1
    } else {
        result
    }
}
