use std::{arch::asm, ffi::c_void};

pub const STD_IN: i32 = 0;
pub const STD_OUT: i32 = 1;
pub const STD_ERR: i32 = 2;

#[inline(always)]
pub fn write(
    file_descriptor: i32,
    buffer_pointer: *const c_void,
    buffer_length_in_bytes: usize,
) -> i32 {
    const WRITE: usize = 1;

    let result: isize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") WRITE => result,
            in("rdi") file_descriptor,
            in("rsi") buffer_pointer,
            in("rdx") buffer_length_in_bytes,
            out("rcx") _,
            out("r11") _,
            options(nostack)
        )
    };
    result as i32
}
