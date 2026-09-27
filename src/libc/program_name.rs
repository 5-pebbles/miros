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

/// Points the program-name exports at argv[0], or at the empty string when there is none.
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

/// Returns the component past the last '/', or the whole string when it has none.
fn base_name(path: *const u8) -> *const u8 {
    let bytes = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    match bytes.iter().rposition(|&byte| byte == b'/') {
        Some(slash_index) => unsafe { path.add(slash_index + 1) },
        None => path,
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::CString, sync::Mutex};

    use super::*;

    // Both set_program_name tests write the process-global cells; cargo test runs them in parallel.
    static SET_PROGRAM_NAME_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn base_name_splits_at_the_last_slash() {
        let path = CString::new("/usr/bin/git").unwrap();
        let base = base_name(path.as_ptr().cast::<u8>());
        let expected = unsafe { path.as_ptr().cast::<u8>().add(9) };
        assert_eq!(base, expected);
    }

    #[test]
    fn base_name_without_a_slash_is_the_whole_path() {
        let path = CString::new("miros").unwrap();
        assert_eq!(
            base_name(path.as_ptr().cast::<u8>()),
            path.as_ptr().cast::<u8>()
        );
    }

    #[test]
    fn a_trailing_slash_yields_an_empty_basename() {
        let path = CString::new("/usr/bin/").unwrap();
        let base = base_name(path.as_ptr().cast::<u8>());
        let expected = unsafe { path.as_ptr().cast::<u8>().add(9) };
        assert_eq!(base, expected);
    }

    #[test]
    fn set_program_name_stores_full_and_short_forms() {
        let _guard = SET_PROGRAM_NAME_LOCK.lock().unwrap();
        let path = CString::new("./examples/bin/program_name").unwrap();
        unsafe { set_program_name(path.as_ptr().cast::<u8>()) };

        let full = unsafe { *PROGNAME_FULL.as_ptr() };
        let short = unsafe { *PROGNAME.as_ptr() };
        assert_eq!(full, path.as_ptr().cast::<u8>().cast_mut());
        assert_eq!(short, unsafe {
            path.as_ptr().cast::<u8>().add(15).cast_mut()
        });
    }

    #[test]
    fn a_missing_argv_zero_yields_the_empty_string() {
        let _guard = SET_PROGRAM_NAME_LOCK.lock().unwrap();
        unsafe { set_program_name(ptr::null()) };
        let full = unsafe { *PROGNAME_FULL.as_ptr() };
        let short = unsafe { *PROGNAME.as_ptr() };
        assert_eq!(unsafe { *full }, 0);
        assert_eq!(unsafe { *short }, 0);
    }
}
