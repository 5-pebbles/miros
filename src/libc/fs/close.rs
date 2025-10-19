use std::{arch::asm, os::fd::RawFd};

use crate::libc::errno::{set_errno, Errno};

#[no_mangle]
unsafe extern "C" fn close(file_descriptor: RawFd) -> i32 {
    let result: isize;

    if file_descriptor == -1 {
        set_errno(Errno::BADF);
        return -1;
    }

    #[cfg(target_arch = "x86_64")]
    {
        const CLOSE: usize = 3;
        asm!(
            "syscall",
            inlateout("rax") CLOSE => result,
            in("rdi") file_descriptor,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags, readonly)
        );
    }

    result as i32
}
