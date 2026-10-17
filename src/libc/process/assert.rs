use core::ffi::{c_char, c_uint};
use std::{
    ffi::CStr,
    io::{Cursor, Write},
};

use crate::libc::{process::abort, stdio::write_stderr};

unsafe fn write_string(string: *const c_char) {
    if !string.is_null() {
        write_stderr(CStr::from_ptr(string).to_bytes());
    }
}

/// Prints `<program>: <file>:<line>: <function>: Assertion `<expr>' failed.` to stderr, then aborts.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn __assert_fail(
    assertion: *const c_char,
    file: *const c_char,
    line: c_uint,
    function: *const c_char,
) -> ! {
    let mut line_buffer = [0u8; 16];
    let mut cursor = Cursor::new(&mut line_buffer[..]);
    let _ = write!(cursor, "{line}");
    let line_length = cursor.position() as usize;

    write_string(crate::libc::program_name::program_name().cast());
    write_stderr(b": ");
    write_string(file);
    write_stderr(b":");
    write_stderr(&line_buffer[..line_length]);
    write_stderr(b": ");
    write_string(function);
    write_stderr(b": Assertion `");
    write_string(assertion);
    write_stderr(b"' failed.\n");

    abort();
}
