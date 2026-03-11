use std::{arch::asm, ffi::c_void};

#[no_mangle]
unsafe extern "C" fn __libc_start_main(
    _main: unsafe extern "C" fn(i32, *const *const u8, *const *const u8) -> i32,
    _argc: i32,
    _argv: *const *const u8,
    _init: *const c_void,
    _fini: *const c_void,
    _rtld_fini: *const c_void,
    _stack_end: *const c_void,
) -> i32 {
    asm!("ud2", options(noreturn, nostack));
}
