use core::ffi::{c_char, c_int};
use std::ptr;

use linkme::distributed_slice;

use crate::libc::interposable::{Bindable, InterposableCell, INTERPOSABLE_CELLS};

// Programs COPY-relocate `optind`, so it needs an interposable cell; the other three are exported for glibc parity.
#[cfg_attr(not(test), export_name = "optarg")]
static mut OPTARG_CELL: *mut c_char = ptr::null_mut();
#[cfg_attr(not(test), export_name = "opterr")]
static mut OPTERR_CELL: c_int = 1;
#[cfg_attr(not(test), export_name = "optopt")]
static mut OPTOPT_CELL: c_int = b'?' as c_int;
#[cfg_attr(not(test), export_name = "optind")]
static mut OPTIND_CELL: c_int = 1;

static OPTARG: InterposableCell<*mut c_char> =
    InterposableCell::new(&["optarg"], &raw mut OPTARG_CELL);
static OPTERR: InterposableCell<c_int> = InterposableCell::new(&["opterr"], &raw mut OPTERR_CELL);
static OPTOPT: InterposableCell<c_int> = InterposableCell::new(&["optopt"], &raw mut OPTOPT_CELL);
static OPTIND: InterposableCell<c_int> = InterposableCell::new(&["optind"], &raw mut OPTIND_CELL);

#[distributed_slice(INTERPOSABLE_CELLS)]
static OPTARG_BINDING: &'static dyn Bindable = &OPTARG;
#[distributed_slice(INTERPOSABLE_CELLS)]
static OPTERR_BINDING: &'static dyn Bindable = &OPTERR;
#[distributed_slice(INTERPOSABLE_CELLS)]
static OPTOPT_BINDING: &'static dyn Bindable = &OPTOPT;
#[distributed_slice(INTERPOSABLE_CELLS)]
static OPTIND_BINDING: &'static dyn Bindable = &OPTIND;

pub(super) fn option_index() -> c_int {
    unsafe { *OPTIND.as_ptr() }
}

pub(super) fn set_option_index(value: c_int) {
    unsafe { *OPTIND.as_ptr() = value };
}

pub(super) fn clear_argument() {
    unsafe { *OPTARG.as_ptr() = ptr::null_mut() };
}

pub(super) fn set_argument(value: *mut c_char) {
    unsafe { *OPTARG.as_ptr() = value };
}

pub(super) fn set_option_character(value: c_int) {
    unsafe { *OPTOPT.as_ptr() = value };
}

pub(super) fn errors_enabled() -> bool {
    unsafe { *OPTERR.as_ptr() != 0 }
}
