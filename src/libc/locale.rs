use core::ffi::{c_char, c_int};

use crate::signature_matches_libc;

// Hum, the C locale is the only one miros has.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn setlocale(_category: c_int, _locale: *const c_char) -> *mut c_char {
    signature_matches_libc!(libc::setlocale(_category, _locale));
    c"C".as_ptr() as *mut c_char
}
