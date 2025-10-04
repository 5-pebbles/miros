use std::ffi::c_void;

use crate::syscall::write::write as syscall_write;

use crate::signature_matches_libc;

unsafe extern "C" fn write(
    file_descriptor: i32,
    buffer_pointer: *const c_void,
    buffer_length_in_bytes: usize,
) -> isize {
    signature_matches_libc!(libc::write(
        file_descriptor,
        buffer_pointer,
        buffer_length_in_bytes
    ));

    syscall_write(file_descriptor, buffer_pointer, buffer_length_in_bytes) as isize
}
