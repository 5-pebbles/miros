use std::ffi::{c_int, c_void};

use crate::libc::net::{net_syscall_pass_through, socklen_t};

net_syscall_pass_through! {
    fn setsockopt(socket: c_int, level: c_int, option_name: c_int, option_value: *const c_void, option_len: socklen_t) -> c_int = Syscall::SetSockOpt;
    fn getsockopt(socket: c_int, level: c_int, option_name: c_int, option_value: *mut c_void, option_len: *mut socklen_t) -> c_int = Syscall::GetSockOpt;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libc::net::socket::socket;

    const AF_INET: c_int = linux_raw_sys::net::AF_INET as c_int;
    const SOCK_STREAM: c_int = linux_raw_sys::net::SOCK_STREAM as c_int;

    #[test]
    fn setsockopt_then_getsockopt_round_trip_on_reuseaddr() {
        let socket_descriptor = unsafe { socket(AF_INET, SOCK_STREAM, 0) };
        let one: c_int = 1;
        assert_eq!(
            unsafe {
                setsockopt(
                    socket_descriptor,
                    libc::SOL_SOCKET,
                    libc::SO_REUSEADDR,
                    (&one as *const c_int).cast(),
                    size_of::<c_int>() as u32,
                )
            },
            0
        );

        let mut value: c_int = 0;
        let mut value_len = size_of::<c_int>() as u32;
        assert_eq!(
            unsafe {
                getsockopt(
                    socket_descriptor,
                    libc::SOL_SOCKET,
                    libc::SO_REUSEADDR,
                    (&mut value as *mut c_int).cast(),
                    &mut value_len,
                )
            },
            0
        );
        assert_eq!(value, 1);
        assert_eq!(unsafe { libc::close(socket_descriptor) }, 0);
    }
}
