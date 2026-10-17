// FIXME: miros ships no message catalogs, so gettext lookups return the message id unchanged.

use core::ffi::{c_char, c_int};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn bindtextdomain(
    _domain_name: *const c_char,
    directory_name: *const c_char,
) -> *mut c_char {
    if directory_name.is_null() {
        c"/usr/share/locale".as_ptr() as *mut c_char
    } else {
        directory_name as *mut c_char
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn dcgettext(
    _domain_name: *const c_char,
    message_id: *const c_char,
    _category: c_int,
) -> *mut c_char {
    message_id as *mut c_char
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn textdomain(domain_name: *const c_char) -> *mut c_char {
    if domain_name.is_null() {
        c"messages".as_ptr() as *mut c_char
    } else {
        domain_name as *mut c_char
    }
}
