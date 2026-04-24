use std::ffi::c_void;

use crate::{
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

pub const STD_IN: i32 = 0;
pub const STD_OUT: i32 = 1;
pub const STD_ERR: i32 = 2;

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn write(
    file_descriptor: i32,
    buffer_pointer: *const c_void,
    buffer_length_in_bytes: usize,
) -> isize {
    signature_matches_libc!(libc::write(
        file_descriptor,
        buffer_pointer.cast(),
        buffer_length_in_bytes
    ));

    syscall!(
        Syscall::Write,
        file_descriptor,
        buffer_pointer,
        buffer_length_in_bytes
    )
}
