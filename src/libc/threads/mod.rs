use std::ffi::c_void;

use crate::{
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

mod key;

// TODO: Store and invoke destructors when thread support is implemented.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn __cxa_thread_atexit_impl(
    _destructor: unsafe extern "C" fn(*mut c_void),
    _object: *mut c_void,
    _dso_symbol: *mut c_void,
) -> i32 {
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn gettid() -> i32 {
    signature_matches_libc!(std::mem::transmute(libc::gettid()));
    let result = syscall!(Syscall::GetTid);
    result as i32
}
