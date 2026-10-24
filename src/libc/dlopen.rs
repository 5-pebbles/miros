use std::ffi::{c_char, c_int, c_void};

use bitbybit::bitfield;

use crate::signature_matches_libc;

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn dlopen(filename: *const c_char, mode: RtldMode) -> *mut c_void {
    signature_matches_libc!(libc::dlopen(filename, std::mem::transmute(mode)));
    todo!()
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn dlmopen(
    namespace: libc::Lmid_t,
    filename: *const c_char,
    mode: RtldMode,
) -> *mut c_void {
    signature_matches_libc!(libc::dlmopen(
        namespace,
        filename,
        std::mem::transmute(mode)
    ));
    todo!()
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn dlclose(handle: *mut c_void) -> c_int {
    signature_matches_libc!(libc::dlclose(handle));
    todo!()
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn dlerror() -> *mut c_char {
    signature_matches_libc!(libc::dlerror());
    todo!()
}

#[bitfield(u32)]
pub struct RtldMode {
    #[bit(0, rw)]
    lazy: bool,
    #[bit(1, rw)]
    now: bool,
    #[bit(2, rw)]
    do_not_load: bool,
    #[bit(3, rw)]
    deep_bind: bool,
    #[bit(8, rw)]
    global: bool,
    #[bit(12, rw)]
    do_not_delete: bool,
}
