use core::ffi::{c_char, c_void};
use std::ptr;

use crate::signature_matches_libc;

// TODO: For now miros resolves every symbol eagerly at load, so there is no runtime table to consult.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void {
    signature_matches_libc!(libc::dlsym(handle, symbol));
    ptr::null_mut()
}
