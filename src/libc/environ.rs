use std::mem::MaybeUninit;

use crate::io_macros::syscall_debug_assert;

#[no_mangle]
#[allow(non_upper_case_globals)]
static mut environ: MaybeUninit<*const *const u8> = MaybeUninit::uninit();

pub unsafe fn set_environ_pointer(environ_pointer: *const *const u8) {
    syscall_debug_assert!((*environ_pointer.sub(1)).is_null());

    #[allow(static_mut_refs)]
    environ.write(environ_pointer);
}

pub unsafe fn get_environ_pointer() -> *const *const u8 {
    #[allow(static_mut_refs)]
    environ.assume_init_read()
}
