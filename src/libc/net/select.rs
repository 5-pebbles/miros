use std::ffi::{c_int, c_void};

use crate::libc::net::net_syscall_pass_through;

// fd_set is a 1024-bit kernel-mutated bitmap; glibc's timeval layout matches the kernel's.
net_syscall_pass_through! {
    fn select(count: c_int, read_fds: *mut c_void, write_fds: *mut c_void, except_fds: *mut c_void, timeout: *mut c_void) -> c_int = Syscall::Select;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libc::net::socket::socketpair;

    const AF_UNIX: c_int = linux_raw_sys::net::AF_UNIX as c_int;
    const SOCK_STREAM: c_int = linux_raw_sys::net::SOCK_STREAM as c_int;

    #[test]
    fn select_reports_readability_after_a_write() {
        let mut pair = [0; 2];
        assert_eq!(unsafe { socketpair(AF_UNIX, SOCK_STREAM, 0, &mut pair) }, 0);

        let mut read_set: libc::fd_set = unsafe { std::mem::zeroed() };
        unsafe { libc::FD_SET(pair[1], &mut read_set) };
        let mut timeout = libc::timeval {
            tv_sec: 1,
            tv_usec: 0,
        };

        let message = b"x";
        assert_eq!(
            unsafe { libc::write(pair[0], message.as_ptr().cast(), 1) },
            1
        );

        let ready = unsafe {
            select(
                pair[1] + 1,
                (&mut read_set as *mut libc::fd_set).cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                (&mut timeout as *mut libc::timeval).cast(),
            )
        };
        assert_eq!(ready, 1);
        assert!(unsafe { libc::FD_ISSET(pair[1], &read_set) });

        assert_eq!(unsafe { libc::close(pair[0]) }, 0);
        assert_eq!(unsafe { libc::close(pair[1]) }, 0);
    }
}
