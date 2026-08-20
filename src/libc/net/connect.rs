use std::ffi::c_int;

use crate::libc::net::{net_syscall_pass_through, sockaddr, socklen_t};

net_syscall_pass_through! {
    fn connect(socket: c_int, address: *const sockaddr, address_len: socklen_t) -> c_int = Syscall::Connect;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libc::{
        errno::errno,
        net::{socket::socket, SOCK_CLOEXEC},
    };

    const AF_INET: c_int = linux_raw_sys::net::AF_INET as c_int;
    const SOCK_STREAM: c_int = linux_raw_sys::net::SOCK_STREAM as c_int;

    fn loopback_address(port: u16) -> linux_raw_sys::net::sockaddr_in {
        linux_raw_sys::net::sockaddr_in {
            sin_family: AF_INET as u16,
            sin_port: port.to_be(),
            sin_addr: linux_raw_sys::net::in_addr {
                s_addr: linux_raw_sys::net::INADDR_LOOPBACK.to_be(),
            },
            __pad: [0; 8],
        }
    }

    fn bound_listener() -> (c_int, linux_raw_sys::net::sockaddr_in) {
        let listener = unsafe { socket(AF_INET, SOCK_STREAM, 0) };
        let address = loopback_address(0);
        assert_eq!(
            unsafe {
                crate::libc::net::listen::bind(
                    listener,
                    (&address as *const linux_raw_sys::net::sockaddr_in).cast(),
                    size_of::<linux_raw_sys::net::sockaddr_in>() as u32,
                )
            },
            0
        );
        assert_eq!(unsafe { crate::libc::net::listen::listen(listener, 5) }, 0);

        let mut bound = loopback_address(0);
        let mut bound_len = size_of::<linux_raw_sys::net::sockaddr_in>() as u32;
        assert_eq!(
            unsafe {
                crate::libc::net::names::getsockname(
                    listener,
                    (&mut bound as *mut linux_raw_sys::net::sockaddr_in).cast(),
                    &mut bound_len,
                )
            },
            0
        );
        assert_eq!(u16::from_be(bound.sin_port) == 0, false);
        (listener, bound)
    }

    #[test]
    fn connect_and_accept4_round_trip_over_loopback() {
        let (listener, bound) = bound_listener();

        let client = unsafe { socket(AF_INET, SOCK_STREAM, 0) };
        assert_eq!(
            unsafe {
                connect(
                    client,
                    (&bound as *const linux_raw_sys::net::sockaddr_in).cast(),
                    size_of::<linux_raw_sys::net::sockaddr_in>() as u32,
                )
            },
            0
        );

        let accepted = unsafe {
            crate::libc::net::accept::accept4(
                listener,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                SOCK_CLOEXEC,
            )
        };
        assert_eq!(accepted == -1, false);
        assert_eq!(
            unsafe { libc::fcntl(accepted, libc::F_GETFD) } & libc::FD_CLOEXEC,
            libc::FD_CLOEXEC
        );

        let message = b"hi";
        assert_eq!(
            unsafe { libc::write(client, message.as_ptr().cast(), message.len()) },
            message.len() as isize
        );
        let mut reply = [0u8; 2];
        assert_eq!(
            unsafe { libc::read(accepted, reply.as_mut_ptr().cast(), reply.len()) },
            reply.len() as isize
        );
        assert_eq!(&reply, b"hi");

        let mut peer = loopback_address(0);
        let mut peer_len = size_of::<linux_raw_sys::net::sockaddr_in>() as u32;
        assert_eq!(
            unsafe {
                crate::libc::net::names::getpeername(
                    accepted,
                    (&mut peer as *mut linux_raw_sys::net::sockaddr_in).cast(),
                    &mut peer_len,
                )
            },
            0
        );
        assert_eq!(peer.sin_family, AF_INET as u16);
        assert_eq!(
            peer.sin_addr.s_addr,
            linux_raw_sys::net::INADDR_LOOPBACK.to_be()
        );

        assert_eq!(unsafe { libc::close(accepted) }, 0);
        assert_eq!(unsafe { libc::close(client) }, 0);
        assert_eq!(unsafe { libc::close(listener) }, 0);
    }

    #[test]
    fn connect_to_a_closed_port_is_refused() {
        let client = unsafe { socket(AF_INET, SOCK_STREAM, 0) };
        assert_eq!(
            unsafe {
                connect(
                    client,
                    (&loopback_address(1) as *const linux_raw_sys::net::sockaddr_in).cast(),
                    size_of::<linux_raw_sys::net::sockaddr_in>() as u32,
                )
            },
            -1
        );
        let current_errno: u32 = unsafe { (&*errno.as_ptr()).into() };
        assert_eq!(current_errno, linux_raw_sys::errno::ECONNREFUSED);
        assert_eq!(unsafe { libc::close(client) }, 0);
    }
}
