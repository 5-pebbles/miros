use std::ffi::c_int;

use crate::libc::net::net_syscall_pass_through;

#[allow(non_camel_case_types)]
pub(crate) type nfds_t = u64;

net_syscall_pass_through! {
    fn poll(fds: *mut linux_raw_sys::general::pollfd, count: nfds_t, timeout_milliseconds: c_int) -> c_int = Syscall::Poll;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libc::net::socket::socketpair;

    const AF_UNIX: c_int = linux_raw_sys::net::AF_UNIX as c_int;
    const SOCK_STREAM: c_int = linux_raw_sys::net::SOCK_STREAM as c_int;

    #[test]
    fn poll_reports_readability_after_a_write() {
        let mut pair = [0; 2];
        assert_eq!(unsafe { socketpair(AF_UNIX, SOCK_STREAM, 0, &mut pair) }, 0);

        let mut watched = linux_raw_sys::general::pollfd {
            fd: pair[1],
            events: libc::POLLIN,
            revents: 0,
        };
        assert_eq!(unsafe { poll(&mut watched, 1, 0) }, 0);

        let message = b"x";
        assert_eq!(
            unsafe { libc::write(pair[0], message.as_ptr().cast(), 1) },
            1
        );
        assert_eq!(unsafe { poll(&mut watched, 1, 1000) }, 1);
        assert_ne!(watched.revents & libc::POLLIN, 0);

        assert_eq!(unsafe { libc::close(pair[0]) }, 0);
        assert_eq!(unsafe { libc::close(pair[1]) }, 0);
    }
}
