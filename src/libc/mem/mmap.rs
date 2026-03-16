use std::{arch::asm, os::fd::RawFd};

use crate::{
    io_macros::syscall_debug_assert,
    libc::mem::{MapFlags, ProtectionFlags},
    signature_matches_libc,
    syscall::Syscall,
};

// TODO: add error handling
#[cfg_attr(not(test), no_mangle)]
pub unsafe extern "C" fn mmap(
    pointer: *mut u8,
    size: usize,
    protection_flags: ProtectionFlags,
    map_flags: MapFlags,
    file_descriptor: RawFd,
    file_offset: usize,
) -> *mut u8 {
    signature_matches_libc!((libc::mmap(
        pointer.cast(),
        size,
        std::mem::transmute(protection_flags),
        std::mem::transmute(map_flags),
        file_descriptor,
        file_offset as i64,
    ))
    .cast());
    let mut result: isize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") Syscall::Mmap as usize => result,
            in("rdi") pointer,
            in("rsi") size,
            in("rdx") protection_flags.raw_value(),
            in("r10") map_flags.raw_value(),
            in("r8") file_descriptor,
            in("r9") file_offset,
            out("rcx") _,
            out("r11") _,
            options(nostack)
        );
    }
    syscall_debug_assert!(result >= 0);
    result as *mut u8
}
