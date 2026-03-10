use std::{
    arch::asm,
    ffi::c_void,
    os::fd::{AsRawFd, BorrowedFd},
};

use crate::{signature_matches_libc, syscall::Syscall};

#[no_mangle]
unsafe extern "C" fn pread64(
    file_descriptor: BorrowedFd<'_>,
    buffer_pointer: *mut c_void,
    buffer_length_in_bytes: usize,
    offset: i64,
) -> isize {
    signature_matches_libc!(libc::pread64(
        std::mem::transmute(file_descriptor),
        buffer_pointer.cast(),
        buffer_length_in_bytes,
        offset
    ));

    #[cfg(target_arch = "x86_64")]
    {
        let result: isize;
        unsafe {
            asm!(
                "syscall",
                inlateout("rax") Syscall::PRead64 as usize => result,
                in("rdi") file_descriptor.as_raw_fd(),
                in("rsi") buffer_pointer,
                in("rdx") buffer_length_in_bytes,
                in("r10") offset,
                out("rcx") _,
                out("r11") _,
                options(nostack)
            )
        };
        result
    }
}
