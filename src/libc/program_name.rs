use std::{
    ffi::CStr,
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

use linkme::distributed_slice;

use crate::libc::interposable::{Bindable, InterposableCell, INTERPOSABLE_CELLS};

static EMPTY_STRING: &[u8] = b"\0";

#[cfg_attr(not(test), export_name = "__progname")]
#[allow(non_upper_case_globals)]
static progname: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());

#[cfg_attr(not(test), export_name = "__progname_full")]
#[allow(non_upper_case_globals)]
static progname_full: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());

static PROGNAME: InterposableCell<*mut u8> = InterposableCell::new(
    &["__progname", "program_invocation_short_name"],
    progname.as_ptr(),
);

static PROGNAME_FULL: InterposableCell<*mut u8> = InterposableCell::new(
    &["__progname_full", "program_invocation_name"],
    progname_full.as_ptr(),
);

#[distributed_slice(INTERPOSABLE_CELLS)]
static PROGNAME_CELL: &'static dyn Bindable = &PROGNAME;

#[distributed_slice(INTERPOSABLE_CELLS)]
static PROGNAME_FULL_CELL: &'static dyn Bindable = &PROGNAME_FULL;

/// Must run before the executable's COPY relocations copy the values out.
pub unsafe fn set_program_name(argument_zero: *const u8) {
    let full = if argument_zero.is_null() {
        EMPTY_STRING.as_ptr()
    } else {
        argument_zero
    };
    AtomicPtr::from_ptr(PROGNAME_FULL.as_ptr()).store(full.cast_mut(), Ordering::Relaxed);
    AtomicPtr::from_ptr(PROGNAME.as_ptr()).store(base_name(full).cast_mut(), Ordering::Relaxed);
}

fn base_name(path: *const u8) -> *const u8 {
    let bytes = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    match bytes.iter().rposition(|&byte| byte == b'/') {
        Some(slash_index) => unsafe { path.add(slash_index + 1) },
        None => path,
    }
}

