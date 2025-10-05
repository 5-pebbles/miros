use std::arch::asm;

#[no_mangle]
unsafe extern "C" fn close(file_descriptor: i32) -> i32 {
    #[cfg(target_arch = "x86_64")]
    {
        const CLOSE: usize = 3;
        let result: isize;
        asm!(
            "syscall",
            inlateout("rax") CLOSE => result,
            in("rdi") file_descriptor,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags, readonly)
        );
        result as i32
    }
}
