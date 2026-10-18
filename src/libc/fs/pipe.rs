use core::ffi::c_int;

use crate::{libc::translate_syscall_result, signature_matches_libc, syscall, syscall::Syscall};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pipe(pipe_file_descriptors: *mut c_int) -> c_int {
    signature_matches_libc!(libc::pipe(pipe_file_descriptors));

    let result = syscall!(Syscall::Pipe, pipe_file_descriptors);
    translate_syscall_result(result) as c_int
}
