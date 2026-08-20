use std::ffi::c_int;

use crate::{
    libc::net::translate_syscall_result,
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
pub(crate) unsafe extern "C" fn socket(
    domain: c_int,
    socket_type: c_int,
    protocol: c_int,
) -> c_int {
    signature_matches_libc!(libc::socket(domain, socket_type, protocol));

    translate_syscall_result(syscall!(Syscall::Socket, domain, socket_type, protocol))
}

#[cfg_attr(not(test), no_mangle)]
pub(crate) unsafe extern "C" fn socketpair(
    domain: c_int,
    socket_type: c_int,
    protocol: c_int,
    socket_vector: *mut [c_int; 2],
) -> c_int {
    signature_matches_libc!(libc::socketpair(
        domain,
        socket_type,
        protocol,
        socket_vector.cast(),
    ));

    translate_syscall_result(syscall!(
        Syscall::SocketPair,
        domain,
        socket_type,
        protocol,
        socket_vector
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libc::{
        errno::errno,
        net::{SOCK_CLOEXEC, SOCK_NONBLOCK},
    };

    const AF_INET: c_int = linux_raw_sys::net::AF_INET as c_int;
    const AF_UNIX: c_int = linux_raw_sys::net::AF_UNIX as c_int;
    const SOCK_STREAM: c_int = linux_raw_sys::net::SOCK_STREAM as c_int;
    const INVALID_DOMAIN: c_int = 4095;

    #[test]
    fn socket_returns_a_usable_descriptor() {
        let descriptor = unsafe { socket(AF_INET, SOCK_STREAM, 0) };
        assert_eq!(descriptor == -1, false);
        assert_eq!(unsafe { libc::close(descriptor) }, 0);
    }

    #[test]
    fn socket_rejects_an_unknown_domain() {
        let result = unsafe { socket(INVALID_DOMAIN, SOCK_STREAM, 0) };
        assert_eq!(result, -1);
        let current_errno: u32 = unsafe { (&*errno.as_ptr()).into() };
        assert_eq!(current_errno, linux_raw_sys::errno::EAFNOSUPPORT);
    }

    #[test]
    fn socketpair_passes_bytes_in_both_directions() {
        let mut pair = [0; 2];
        assert_eq!(unsafe { socketpair(AF_UNIX, SOCK_STREAM, 0, &mut pair) }, 0);

        let request = b"ping";
        assert_eq!(
            unsafe { libc::write(pair[0], request.as_ptr().cast(), request.len()) },
            request.len() as isize
        );
        let mut reply = [0u8; 4];
        assert_eq!(
            unsafe { libc::read(pair[1], reply.as_mut_ptr().cast(), reply.len()) },
            reply.len() as isize
        );
        assert_eq!(&reply, b"ping");

        assert_eq!(
            unsafe { libc::write(pair[1], request.as_ptr().cast(), request.len()) },
            request.len() as isize
        );
        assert_eq!(
            unsafe { libc::read(pair[0], reply.as_mut_ptr().cast(), reply.len()) },
            reply.len() as isize
        );
        assert_eq!(&reply, b"ping");

        assert_eq!(unsafe { libc::close(pair[0]) }, 0);
        assert_eq!(unsafe { libc::close(pair[1]) }, 0);
    }

    #[test]
    fn socketpair_socket_type_flags_reach_the_descriptors() {
        let mut pair = [0; 2];
        assert_eq!(
            unsafe {
                socketpair(
                    AF_UNIX,
                    SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC,
                    0,
                    &mut pair,
                )
            },
            0
        );

        let status_flags = unsafe { libc::fcntl(pair[0], libc::F_GETFL) };
        assert_ne!(status_flags & libc::O_NONBLOCK, 0);
        assert_ne!(
            unsafe { libc::fcntl(pair[0], libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );

        assert_eq!(unsafe { libc::close(pair[0]) }, 0);
        assert_eq!(unsafe { libc::close(pair[1]) }, 0);
    }
}
