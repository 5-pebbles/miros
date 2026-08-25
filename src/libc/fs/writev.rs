use std::ffi::c_int;

use crate::{libc::translate_syscall_result, signature_matches_libc, syscall, syscall::Syscall};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn writev(
    file_descriptor: c_int,
    io_vectors: *const libc::iovec,
    count: c_int,
) -> isize {
    signature_matches_libc!(libc::writev(file_descriptor, io_vectors, count));

    let result = syscall!(Syscall::WriteV, file_descriptor, io_vectors, count);
    translate_syscall_result(result)
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Read, os::unix::io::AsRawFd};

    use super::*;

    #[test]
    fn writev_concatenates_segments() {
        let path = std::env::temp_dir().join(format!("miros_writev_{}", std::process::id()));
        let file = File::create(&path).unwrap();
        let io_vectors = [
            libc::iovec {
                iov_base: b"hello ".as_ptr().cast_mut().cast(),
                iov_len: 6,
            },
            libc::iovec {
                iov_base: b"world".as_ptr().cast_mut().cast(),
                iov_len: 5,
            },
        ];

        let written = unsafe { writev(file.as_raw_fd(), io_vectors.as_ptr(), 2) };
        drop(file);

        assert_eq!(written, 11);
        let mut content = String::new();
        File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content, "hello world");
        std::fs::remove_file(path).ok();
    }
}
