use core::ffi::{c_int, c_uint};

use crate::{libc::translate_syscall_result, signature_matches_libc, syscall, syscall::Syscall};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn splice(
    input_file_descriptor: c_int,
    input_offset: *mut libc::loff_t,
    output_file_descriptor: c_int,
    output_offset: *mut libc::loff_t,
    length: usize,
    flags: c_uint,
) -> isize {
    signature_matches_libc!(libc::splice(
        input_file_descriptor,
        input_offset,
        output_file_descriptor,
        output_offset,
        length,
        flags
    ));

    let result = syscall!(
        Syscall::Splice,
        input_file_descriptor,
        input_offset,
        output_file_descriptor,
        output_offset,
        length,
        flags
    );
    translate_syscall_result(result)
}
