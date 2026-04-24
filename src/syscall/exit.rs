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
