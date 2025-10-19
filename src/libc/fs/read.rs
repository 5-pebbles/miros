use std::os::fd::AsRawFd;
use std::{arch::asm, ffi::c_void, os::fd::BorrowedFd};

use crate::signature_matches_libc;

#[no_mangle]
unsafe extern "C" fn read(
    file_descriptor: BorrowedFd<'_>,
    buffer_pointer: *mut c_void,
    buffer_length_in_bytes: usize,
) -> isize {
    signature_matches_libc!(libc::read(
        std::mem::transmute(file_descriptor),
        buffer_pointer,
        buffer_length_in_bytes
    ));

    #[cfg(target_arch = "x86_64")]
    {
        const READ: usize = 0;

        let result: isize;
        unsafe {
            asm!(
                "syscall",
                inlateout("rax") READ => result,
                in("rdi") file_descriptor.as_raw_fd(),
                in("rsi") buffer_pointer,
                in("rdx") buffer_length_in_bytes,
                out("rcx") _,
                out("r11") _,
                options(nostack)
            )
        };
        result
    }
}
