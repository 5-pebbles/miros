use std::ffi::{c_int, c_void};

use crate::libc::net::{net_syscall_pass_through, sockaddr, socklen_t};

// send is sendto without a destination; recv is recvfrom without a source address.
net_syscall_pass_through! {
    fn sendto(socket: c_int, buffer: *const c_void, length: usize, flags: c_int, destination_address: *const sockaddr, address_len: socklen_t) -> isize = Syscall::SendTo;
    fn recvfrom(socket: c_int, buffer: *mut c_void, length: usize, flags: c_int, address: *mut sockaddr, address_len: *mut socklen_t) -> isize = Syscall::RecvFrom;
    fn send(socket: c_int, buffer: *const c_void, length: usize, flags: c_int) -> isize {
        sendto(socket, buffer, length, flags, std::ptr::null(), 0)
    }
    fn recv(socket: c_int, buffer: *mut c_void, length: usize, flags: c_int) -> isize {
        recvfrom(socket, buffer, length, flags, std::ptr::null_mut(), std::ptr::null_mut())
    }
    fn sendmsg(socket: c_int, message: *const linux_raw_sys::net::msghdr, flags: c_int) -> isize = Syscall::SendMsg;
    fn recvmsg(socket: c_int, message: *mut linux_raw_sys::net::msghdr, flags: c_int) -> isize = Syscall::RecvMsg;
    fn shutdown(socket: c_int, how: c_int) -> c_int = Syscall::Shutdown;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libc::net::socket::socketpair;

    const AF_UNIX: c_int = linux_raw_sys::net::AF_UNIX as c_int;
    const SOCK_STREAM: c_int = linux_raw_sys::net::SOCK_STREAM as c_int;

    fn stream_pair() -> [c_int; 2] {
        let mut pair = [0; 2];
        assert_eq!(unsafe { socketpair(AF_UNIX, SOCK_STREAM, 0, &mut pair) }, 0);
        pair
    }

    #[test]
    fn send_recv_round_trip() {
        let pair = stream_pair();
        let message = b"hello";
        assert_eq!(
            unsafe { send(pair[0], message.as_ptr().cast(), message.len(), 0) },
            message.len() as isize
        );
        let mut reply = [0u8; 5];
        assert_eq!(
            unsafe { recv(pair[1], reply.as_mut_ptr().cast(), reply.len(), 0) },
            reply.len() as isize
        );
        assert_eq!(&reply, b"hello");
        assert_eq!(unsafe { libc::close(pair[0]) }, 0);
        assert_eq!(unsafe { libc::close(pair[1]) }, 0);
    }

    #[test]
    fn sendto_recvfrom_with_null_address_match_send_recv() {
        let pair = stream_pair();
        let message = b"xy";
        assert_eq!(
            unsafe {
                sendto(
                    pair[0],
                    message.as_ptr().cast(),
                    message.len(),
                    0,
                    std::ptr::null(),
                    0,
                )
            },
            message.len() as isize
        );
        let mut reply = [0u8; 2];
        assert_eq!(
            unsafe {
                recvfrom(
                    pair[1],
                    reply.as_mut_ptr().cast(),
                    reply.len(),
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            reply.len() as isize
        );
        assert_eq!(&reply, b"xy");
        assert_eq!(unsafe { libc::close(pair[0]) }, 0);
        assert_eq!(unsafe { libc::close(pair[1]) }, 0);
    }

    #[test]
    fn shutdown_write_side_produces_eof_on_the_peer() {
        let pair = stream_pair();
        let message = b"z";
        assert_eq!(
            unsafe { send(pair[0], message.as_ptr().cast(), message.len(), 0) },
            1
        );
        assert_eq!(unsafe { shutdown(pair[0], libc::SHUT_WR) }, 0);

        let mut reply = [0u8; 2];
        assert_eq!(
            unsafe { recv(pair[1], reply.as_mut_ptr().cast(), reply.len(), 0) },
            1
        );
        assert_eq!(
            unsafe { recv(pair[1], reply.as_mut_ptr().cast(), reply.len(), 0) },
            0
        );

        assert_eq!(unsafe { libc::close(pair[0]) }, 0);
        assert_eq!(unsafe { libc::close(pair[1]) }, 0);
    }
}
