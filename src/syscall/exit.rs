use std::arch::asm;

use super::Syscall;

const CODE_ADDEND: usize = 22200;

pub const EXIT_UNKNOWN_RELOCATION: usize = CODE_ADDEND + 1;

#[inline(always)]
pub fn exit(code: usize) -> ! {
    unsafe {
        asm!(
            "syscall",
            in("rax") Syscall::Exit as usize,
            in("rdi") code,
            options(noreturn)
        )
    }
}

/// Unmap `[base, base + length)` then exit the thread, touching no stack between the two syscalls.
pub unsafe fn munmap_and_exit(base: *mut u8, length: usize) -> ! {
    asm!(
        "syscall",              // munmap(base, length)
        "mov rax, {exit_number}",
        "xor edi, edi",         // exit code 0
        "syscall",              // exit(0)
        in("rax") Syscall::MunMap as usize,
        in("rdi") base,
        in("rsi") length,
        exit_number = const Syscall::Exit as usize,
        options(noreturn, nostack),
    )
}
