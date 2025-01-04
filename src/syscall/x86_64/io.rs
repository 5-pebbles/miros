use std::arch::asm;

use crate::io_macros::syscall_debug_assert;

pub const STD_IN: i32 = 0;
pub const STD_OUT: i32 = 1;
pub const STD_ERR: i32 = 2;

#[inline(always)]
pub fn write(fd: i32, s: &str) {
    const WRITE: usize = 1;

    let result: isize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") WRITE => result,
            in("rdi") fd,
            in("rsi") s.as_ptr(),
            in("rdx") s.len(),
            out("rcx") _,
            out("r11") _,
            options(nostack)
        )
    };
}
