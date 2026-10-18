mod argument;
mod diagnostics;
mod long_option;
mod parser;
mod permutation;
mod short_option;
mod state;
mod syntax;

use core::ffi::{c_char, c_int};
use std::{ffi::CStr, ptr, slice};

use parser::Parser;
use syntax::long_option_table;

use crate::signature_matches_libc;

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn getopt_long(
    argc: c_int,
    argv: *const *mut c_char,
    optstring: *const c_char,
    longopts: *const libc::option,
    longindex: *mut c_int,
) -> c_int {
    signature_matches_libc!(libc::getopt_long(
        argc, argv, optstring, longopts, longindex
    ));

    let argv = slice::from_raw_parts_mut(argv.cast_mut(), argc.max(0) as usize);
    let option_string = CStr::from_ptr(optstring).to_bytes();
    let argument_zero = argv.first().copied().unwrap_or(ptr::null_mut());

    Parser::new(
        argv,
        option_string,
        long_option_table(longopts),
        argument_zero,
    )
    .next(longindex)
}
