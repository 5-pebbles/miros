use std::ffi::{c_int, c_void};

use crate::{libc::translate_syscall_result, signature_matches_libc, syscall, syscall::Syscall};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn madvise(address: *mut c_void, length: usize, advice: c_int) -> c_int {
    signature_matches_libc!(libc::madvise(address, length, advice));

    let result = syscall!(Syscall::MAdvise, address, length, advice);
    translate_syscall_result(result) as c_int
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn madvise_dontneed_on_mapping() {
        let length = 8192;
        let mapping = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                length,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        assert_ne!(mapping, libc::MAP_FAILED);

        let result = unsafe { madvise(mapping, length, libc::MADV_DONTNEED) };

        assert_eq!(result, 0);
        unsafe { libc::munmap(mapping, length) };
    }
}
