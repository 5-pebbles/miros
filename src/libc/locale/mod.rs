mod ctype;
mod multibyte;

use core::ffi::{c_char, c_int};
use std::ffi::CStr;

use crate::signature_matches_libc;

// FIXME: only the C locale exists; setlocale ignores its arguments and always reports "C".
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn setlocale(_category: c_int, _locale: *const c_char) -> *mut c_char {
    signature_matches_libc!(libc::setlocale(_category, _locale));
    c"C".as_ptr() as *mut c_char
}

const CODESET: c_int = 14;
const RADIXCHAR: c_int = 0x10000;

// FIXME: only the C locale exists, so every item returns an ASCII or default value.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn nl_langinfo(item: c_int) -> *mut c_char {
    signature_matches_libc!(libc::nl_langinfo(item));

    let value: &CStr = match item {
        CODESET => c"ANSI_X3.4-1968",
        RADIXCHAR => c".",
        _ => c"",
    };
    value.as_ptr() as *mut c_char
}
