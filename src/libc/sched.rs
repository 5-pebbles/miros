use core::ffi::c_int;
use std::ptr;

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn sched_yield() -> c_int {
    signature_matches_libc!(libc::sched_yield());
    let result = syscall!(Syscall::SchedYield);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn sched_getaffinity(
    pid: libc::pid_t,
    cpu_set_size: libc::size_t,
    cpu_set: *mut libc::cpu_set_t,
) -> c_int {
    signature_matches_libc!(libc::sched_getaffinity(pid, cpu_set_size, cpu_set));
    let result = syscall!(Syscall::SchedGetAffinity, pid, cpu_set_size, cpu_set);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        return -1;
    }
    // The kernel writes only as many bytes as the mask needs; glibc zeroes the rest of the caller's set.
    let bytes_written = result as usize;
    ptr::write_bytes(
        (cpu_set as *mut u8).add(bytes_written),
        0,
        cpu_set_size - bytes_written,
    );
    0
}
