use core::ffi::{c_int, c_ulong, c_void};

use crate::{libc::translate_syscall_result, signature_matches_libc, syscall, syscall::Syscall};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn ioctl(file_descriptor: c_int, request: c_ulong, mut argument: ...) -> c_int {
    signature_matches_libc!(libc::ioctl(file_descriptor, request, argument));

    let argument = argument.next_arg::<*mut c_void>();
    let result = syscall!(Syscall::IoCtl, file_descriptor, request as usize, argument);
    translate_syscall_result(result) as c_int
}
