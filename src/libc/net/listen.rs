use std::ffi::c_int;

use crate::libc::net::{net_syscall_pass_through, sockaddr, socklen_t};

net_syscall_pass_through! {
    fn bind(socket: c_int, address: *const sockaddr, address_len: socklen_t) -> c_int = Syscall::Bind;
    fn listen(socket: c_int, backlog: c_int) -> c_int = Syscall::Listen;
}
