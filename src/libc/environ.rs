use std::{
    ffi::CStr,
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

use linkme::distributed_slice;

use crate::{
    io_macros::syscall_debug_assert,
    libc::interposable::{Bindable, InterposableCell, INTERPOSABLE_CELLS},
    signature_matches_libc,
    start::environment_variables::EnvironmentIter,
};

#[cfg_attr(not(test), export_name = "__environ")]
#[allow(non_upper_case_globals)]
static environ: AtomicPtr<*mut u8> = AtomicPtr::new(ptr::null_mut());

static ENVIRON: InterposableCell<*mut *mut u8> =
    InterposableCell::new("__environ", environ.as_ptr());

#[distributed_slice(INTERPOSABLE_CELLS)]
static ENVIRON_CELL: &'static dyn Bindable = &ENVIRON;

pub unsafe fn set_environ_pointer(environ_pointer: *mut *mut u8) {
    syscall_debug_assert!((*environ_pointer.sub(1)).is_null());

    AtomicPtr::from_ptr(ENVIRON.as_ptr()).store(environ_pointer, Ordering::Relaxed);
}

pub unsafe fn get_environ_pointer() -> *mut *mut u8 {
    AtomicPtr::from_ptr(ENVIRON.as_ptr()).load(Ordering::Relaxed)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn getenv(variable_name_pointer: *const u8) -> *const u8 {
    signature_matches_libc!(libc::getenv(variable_name_pointer.cast()).cast());

    let variable_name = CStr::from_ptr(variable_name_pointer.cast())
        .to_str()
        .unwrap();
    EnvironmentIter::new(get_environ_pointer())
        .find_map(|(name, value)| {
            if name == variable_name {
                Some(value.as_ptr())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Sequential: rebinding mid-flight would misdirect concurrent stores.
    #[test]
    fn environ_set_get_and_rebind() {
        let mut own_backing: [*mut u8; 2] = [ptr::null_mut(); 2];
        let own_array = unsafe { own_backing.as_mut_ptr().add(1) };
        unsafe { set_environ_pointer(own_array) };
        assert_eq!(unsafe { get_environ_pointer() }, own_array);

        let mut copied_cell: *mut *mut u8 = ptr::null_mut();
        ENVIRON.rebind(&raw mut copied_cell);

        let mut other_backing: [*mut u8; 2] = [ptr::null_mut(); 2];
        let other_array = unsafe { other_backing.as_mut_ptr().add(1) };
        unsafe { set_environ_pointer(other_array) };
        assert_eq!(copied_cell, other_array);
        assert_eq!(unsafe { get_environ_pointer() }, other_array);
        assert_eq!(environ.load(Ordering::Relaxed), own_array);

        ENVIRON.rebind(environ.as_ptr());
    }
}
