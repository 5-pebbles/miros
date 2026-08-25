use std::{ffi::c_char, ptr};

use crate::{
    libc::{
        alloc::{free, malloc},
        errno::{set_errno, Errno},
        translate_syscall_result,
    },
    signature_matches_libc, syscall,
    syscall::Syscall,
};

const PATH_MAX: usize = 4096;

/// A null `buffer` allocates from malloc (`PATH_MAX` when `size` is 0).
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn getcwd(buffer: *mut c_char, size: usize) -> *mut c_char {
    signature_matches_libc!(libc::getcwd(buffer, size));

    let allocated = buffer.is_null();
    let (buffer, size) = if allocated {
        let size = if size == 0 { PATH_MAX } else { size };
        let buffer = malloc(size).cast::<c_char>();
        if buffer.is_null() {
            return ptr::null_mut();
        }
        (buffer, size)
    } else if size == 0 {
        set_errno(Errno::INVAL);
        return ptr::null_mut();
    } else {
        (buffer, size)
    };

    if translate_syscall_result(syscall!(Syscall::GetCwd, buffer, size)) == -1 {
        if allocated {
            free(buffer.cast());
        }
        return ptr::null_mut();
    }
    buffer
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    use super::*;
    use crate::libc::errno::errno;

    #[test]
    fn getcwd_caller_buffer_roundtrip() {
        let mut buffer = [0u8; PATH_MAX];

        let result = unsafe { getcwd(buffer.as_mut_ptr().cast(), buffer.len()) };

        assert_eq!(result, buffer.as_mut_ptr().cast());
        let path = unsafe { CStr::from_ptr(result) };
        assert_eq!(
            path.to_str().unwrap(),
            std::env::current_dir().unwrap().to_str().unwrap()
        );
    }

    #[test]
    fn getcwd_rejects_zero_size_caller_buffer() {
        let mut buffer = [0u8; 16];

        let result = unsafe { getcwd(buffer.as_mut_ptr().cast(), 0) };

        assert!(result.is_null());
        assert_eq!(errno.get(), Errno::INVAL);
    }
}
