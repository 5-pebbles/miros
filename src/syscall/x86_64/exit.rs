use std::arch::asm;

const CODE_ADDEND: usize = 22200;

pub const EXIT_UNKNOWN_RELOCATION: usize = CODE_ADDEND + 1;

#[inline(always)]
pub fn exit(code: usize) -> ! {
    const EXIT: usize = 60;

    unsafe {
        asm!(
            "syscall",
            in("rax") EXIT,
            in("rdi") code,
            options(noreturn)
        )
    }
}
