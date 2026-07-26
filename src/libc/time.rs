use core::ffi::{c_int, c_void};

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

// glibc routes these through the vDSO; the raw syscall is correct, just without the vDSO fast path.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn clock_gettime(clock_id: c_int, time: *mut libc::timespec) -> c_int {
    signature_matches_libc!(libc::clock_gettime(clock_id, time));
    let result = syscall!(Syscall::ClockGetTime, clock_id, time);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn gettimeofday(time: *mut libc::timeval, timezone: *mut c_void) -> c_int {
    signature_matches_libc!(libc::gettimeofday(time, timezone.cast()));
    let result = syscall!(Syscall::GetTimeOfDay, time, timezone);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}
