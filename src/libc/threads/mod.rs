use crate::signature_matches_libc;
use std::arch::asm;

#[no_mangle]
unsafe extern "C" fn gettid() -> i32 {
    signature_matches_libc!(std::mem::transmute(libc::gettid()));
    let result: isize;

    #[cfg(target_arch = "x86_64")]
    {
        const GETPID: usize = 186;
        asm!(
            "syscall",
            inlateout("rax") GETPID => result,
            out("rcx") _,
            out("r11") _,
            options(nostack),
        )
    }
    result as i32
}
