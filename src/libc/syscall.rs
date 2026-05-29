use std::ffi::c_long;

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::syscall,
};

// The kernel returns errors as `-errno` in this range; everything else is a
// genuine result (including valid negative offsets and pointers).
const ERROR_RANGE: std::ops::RangeInclusive<isize> = -4095..=-1;

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn syscall(number: c_long, mut args: ...) -> c_long {
    signature_matches_libc!(libc::syscall(number, args));

    // Always pull six args; the ABI ignores registers a syscall doesn't read.
    let result: isize = syscall!(
        number,
        args.arg::<c_long>(),
        args.arg::<c_long>(),
        args.arg::<c_long>(),
        args.arg::<c_long>(),
        args.arg::<c_long>(),
        args.arg::<c_long>(),
    );

    if ERROR_RANGE.contains(&result) {
        set_errno(Errno((-result) as u32));
        -1
    } else {
        result as c_long
    }
}
