use std::{arch::asm, ffi::c_void};

use crate::{signature_matches_libc, syscall::Syscall};

mod key;

// TODO: Store and invoke destructors when thread support is implemented.
#[no_mangle]
unsafe extern "C" fn __cxa_thread_atexit_impl(
    _destructor: unsafe extern "C" fn(*mut c_void),
    _object: *mut c_void,
    _dso_symbol: *mut c_void,
) -> i32 {
    0
}

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
