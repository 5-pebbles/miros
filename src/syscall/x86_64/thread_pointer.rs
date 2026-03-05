use std::{arch::asm, ffi::c_void};

use super::Syscall;
use crate::io_macros::syscall_debug_assert;

#[inline(always)]
pub unsafe fn set_thread_pointer(new_pointer: *mut c_void) {
    const ARCH_SET_FS: usize = 4098;

    asm!(
        "syscall",
        in("rax") Syscall::ArchPrctl as usize,
        in("rdi") ARCH_SET_FS,
        in("rsi") new_pointer,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    syscall_debug_assert!(*new_pointer.cast::<*mut c_void>() == new_pointer);
    syscall_debug_assert!(get_thread_pointer() == new_pointer);
}

#[inline(always)]
pub unsafe fn get_thread_pointer() -> *mut c_void {
    let pointer;
    asm!(
        "mov {}, fs:0",
        out(reg) pointer,
        options(nostack, preserves_flags, readonly)
    );
    pointer
}
