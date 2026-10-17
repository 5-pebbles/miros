use core::ffi::{c_char, c_int};
use std::{ffi::CStr, slice};

/// The argument at `index`; `index` must be in range.
pub(super) fn element(argv: &[*mut c_char], index: c_int) -> *mut c_char {
    *argv.get(index as usize).unwrap()
}

/// A non-empty argument that starts with `-`.
pub(super) fn is_option(argument: *mut c_char) -> bool {
    if argument.is_null() {
        return false;
    }
    // SAFETY: argv entries are NUL-terminated.
    let bytes = unsafe { CStr::from_ptr(argument) }.to_bytes();
    bytes.first() == Some(&b'-') && bytes.len() > 1
}

/// The `--` separator, after which every argument is treated as a non-option.
pub(super) fn is_separator(argument: *mut c_char) -> bool {
    // SAFETY: argv entries are NUL-terminated.
    !argument.is_null() && unsafe { CStr::from_ptr(argument) }.to_bytes() == b"--"
}

/// Views the option array as a slice, or `None` if it is absent. The array ends at the first entry with a null name.
pub(super) unsafe fn long_option_table(
    options: *const libc::option,
) -> Option<&'static [libc::option]> {
    if options.is_null() {
        return None;
    }

    let mut count = 0;
    while !(*options.add(count)).name.is_null() {
        count += 1;
    }
    Some(slice::from_raw_parts(options, count))
}
