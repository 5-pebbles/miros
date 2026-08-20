use std::ffi::c_int;

use crate::libc::net::{net_syscall_pass_through, sockaddr, socklen_t};

net_syscall_pass_through! {
    fn accept(socket: c_int, address: *mut sockaddr, address_len: *mut socklen_t) -> c_int = Syscall::Accept;
    fn accept4(socket: c_int, address: *mut sockaddr, address_len: *mut socklen_t, flags: c_int) -> c_int = Syscall::Accept4;
}
