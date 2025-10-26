use std::{
    env,
    ffi::{CStr, CString, OsStr},
    mem::MaybeUninit,
};

use crate::{
    io_macros::syscall_debug_assert,
    signature_matches_libc,
    start::environment_variables::{self, EnvironmentIter},
};

#[no_mangle]
#[allow(non_upper_case_globals)]
static mut environ: MaybeUninit<*mut *mut u8> = MaybeUninit::uninit();

pub unsafe fn set_environ_pointer(environ_pointer: *mut *mut u8) {
    syscall_debug_assert!((*environ_pointer.sub(1)).is_null());

    #[allow(static_mut_refs)]
    environ.write(environ_pointer);
}

pub unsafe fn get_environ_pointer() -> *mut *mut u8 {
    #[allow(static_mut_refs)]
    environ.assume_init_read()
}

#[no_mangle]
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
