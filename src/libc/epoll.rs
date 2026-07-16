use core::ffi::c_int;

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn epoll_create1(flags: c_int) -> c_int {
    signature_matches_libc!(libc::epoll_create1(flags));
    let result = syscall!(Syscall::EpollCreate1, flags);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        result as c_int
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn epoll_ctl(
    epoll_fd: c_int,
    operation: c_int,
    target_fd: c_int,
    event: *mut libc::epoll_event,
) -> c_int {
    signature_matches_libc!(libc::epoll_ctl(epoll_fd, operation, target_fd, event));
    let result = syscall!(Syscall::EpollCtl, epoll_fd, operation, target_fd, event);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn epoll_wait(
    epoll_fd: c_int,
    events: *mut libc::epoll_event,
    max_events: c_int,
    timeout: c_int,
) -> c_int {
    signature_matches_libc!(libc::epoll_wait(epoll_fd, events, max_events, timeout));
    let result = syscall!(Syscall::EpollWait, epoll_fd, events, max_events, timeout);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        result as c_int
    }
}
