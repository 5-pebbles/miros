use std::ffi::c_int;

use crate::libc::net::{net_syscall_pass_through, sockaddr, socklen_t};

net_syscall_pass_through! {
    fn getsockname(socket: c_int, address: *mut sockaddr, address_len: *mut socklen_t) -> c_int = Syscall::GetSockName;
    fn getpeername(socket: c_int, address: *mut sockaddr, address_len: *mut socklen_t) -> c_int = Syscall::GetPeerName;
}
