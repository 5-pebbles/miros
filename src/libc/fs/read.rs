use std::{
    arch::asm,
    ffi::c_void,
    os::fd::{AsRawFd, BorrowedFd},
};

use crate::{signature_matches_libc, syscall::Syscall};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn read(
    file_descriptor: BorrowedFd<'_>,
    buffer_pointer: *mut c_void,
    buffer_length_in_bytes: usize,
) -> isize {
    signature_matches_libc!(libc::read(
        std::mem::transmute(file_descriptor),
        buffer_pointer.cast(),
        buffer_length_in_bytes
    ));

    #[cfg(target_arch = "x86_64")]
    {
        let result: isize;
        unsafe {
            asm!(
                "syscall",
                inlateout("rax") Syscall::Read as usize => result,
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
