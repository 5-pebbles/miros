use std::{ffi::c_int, ptr};

use crate::{libc::translate_syscall_result, signature_matches_libc, syscall, syscall::Syscall};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn sched_yield() -> c_int {
    signature_matches_libc!(libc::sched_yield());

    let result = syscall!(Syscall::SchedYield);
    translate_syscall_result(result) as c_int
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn sched_getaffinity(
    pid: c_int,
    cpu_set_size: usize,
    cpu_set: *mut libc::cpu_set_t,
) -> c_int {
    signature_matches_libc!(libc::sched_getaffinity(pid, cpu_set_size, cpu_set));

    let result = translate_syscall_result(syscall!(
        Syscall::SchedGetAffinity,
        pid,
        cpu_set_size,
        cpu_set
    ));
    if result < 0 {
        return -1;
    }
    // The kernel writes only as many bytes as its mask needs; zero the rest of the caller's set like glibc.
    let bytes_written = result as usize;
    ptr::write_bytes(
        cpu_set.cast::<u8>().add(bytes_written),
        0,
        cpu_set_size - bytes_written,
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sched_yield_succeeds() {
        assert_eq!(unsafe { sched_yield() }, 0);
    }

    #[test]
    fn sched_getaffinity_sets_bits_and_zeroes_tail() {
        let size = std::mem::size_of::<libc::cpu_set_t>();
        let mut prefilled = [0xFFu8; std::mem::size_of::<libc::cpu_set_t>()];
        let mut cleared = [0u8; std::mem::size_of::<libc::cpu_set_t>()];

        assert_eq!(
            unsafe { sched_getaffinity(0, size, prefilled.as_mut_ptr().cast()) },
            0
        );
        assert_eq!(
            unsafe { sched_getaffinity(0, size, cleared.as_mut_ptr().cast()) },
            0
        );

        // Same result regardless of pre-fill: the tail past the kernel mask is zeroed.
        assert_eq!(prefilled, cleared);
        let bits: u32 = prefilled.iter().map(|byte| byte.count_ones()).sum();
        assert!(bits > 0);
    }
}
