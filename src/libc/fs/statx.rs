use core::{
    ffi::{c_char, c_int, c_uint},
    ptr::NonNull,
};

use crate::{libc::translate_syscall_result, signature_matches_libc, syscall, syscall::Syscall};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn statx(
    directory_fd: c_int,
    pathname: *const c_char,
    flags: c_int,
    mask: c_uint,
    status: Option<NonNull<libc::statx>>,
) -> c_int {
    signature_matches_libc!(libc::statx(
        directory_fd,
        pathname,
        flags,
        mask,
        std::mem::transmute(status)
    ));

    let status_pointer = status.map_or(core::ptr::null_mut(), NonNull::as_ptr);
    let result = syscall!(
        Syscall::Statx,
        directory_fd,
        pathname,
        flags,
        mask,
        status_pointer
    );
    translate_syscall_result(result) as c_int
}
