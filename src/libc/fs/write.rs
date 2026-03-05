use std::{arch::asm, ffi::c_void};

use crate::{signature_matches_libc, syscall::Syscall};

pub const STD_IN: i32 = 0;
pub const STD_OUT: i32 = 1;
pub const STD_ERR: i32 = 2;

#[no_mangle]
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

    let result: isize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") Syscall::Write as usize => result,
            in("rdi") file_descriptor,
            in("rsi") buffer_pointer,
            in("rdx") buffer_length_in_bytes,
            out("rcx") _,
            out("r11") _,
            options(nostack)
        )
    };
    result
}
