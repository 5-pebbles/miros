use crate::{signature_matches_libc, syscall::Syscall};
use std::arch::asm;

mod key;

#[no_mangle]
unsafe extern "C" fn gettid() -> i32 {
    signature_matches_libc!(std::mem::transmute(libc::gettid()));
    let result: isize;

    #[cfg(target_arch = "x86_64")]
    {
        asm!(
            "syscall",
            inlateout("rax") Syscall::GetTid as usize => result,
            out("rcx") _,
            out("r11") _,
            options(nostack),
        )
    }
    result as i32
}
