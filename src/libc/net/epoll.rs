use std::ffi::c_int;

use crate::libc::net::net_syscall_pass_through;

// The kernel ignores epoll_create's size argument (since 2.6.8); it must only be positive.
net_syscall_pass_through! {
    fn epoll_create1(flags: c_int) -> c_int = Syscall::EpollCreate1;
    fn epoll_create(_size: c_int) -> c_int {
        epoll_create1(0)
    }
    fn epoll_ctl(epoll_descriptor: c_int, operation: c_int, target: c_int, event: *mut linux_raw_sys::general::epoll_event) -> c_int = Syscall::EpollCtl;
    fn epoll_wait(epoll_descriptor: c_int, events: *mut linux_raw_sys::general::epoll_event, max_events: c_int, timeout_milliseconds: c_int) -> c_int = Syscall::EpollWait;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libc::net::eventfd::eventfd;

    #[test]
    fn epoll_wait_reports_a_ready_eventfd() {
        let epoll_descriptor = unsafe { epoll_create1(0) };
        assert_eq!(epoll_descriptor == -1, false);
        let eventfd_descriptor = unsafe { eventfd(0, 0) };

        let mut interest = linux_raw_sys::general::epoll_event {
            events: linux_raw_sys::general::EPOLLIN,
            data: 0xAB,
        };
        assert_eq!(
            unsafe {
                epoll_ctl(
                    epoll_descriptor,
                    linux_raw_sys::general::EPOLL_CTL_ADD as c_int,
                    eventfd_descriptor,
                    &mut interest,
                )
            },
            0
        );

        let one: u64 = 1;
        assert_eq!(
            unsafe { libc::write(eventfd_descriptor, (&one as *const u64).cast(), 8) },
            8
        );

        let mut ready = [linux_raw_sys::general::epoll_event { events: 0, data: 0 }; 4];
        let ready_count = unsafe {
            epoll_wait(
                epoll_descriptor,
                ready.as_mut_ptr(),
                ready.len() as c_int,
                1000,
            )
        };
        assert_eq!(ready_count, 1);
        let first_ready = ready[0];
        let (ready_data, ready_events) = (first_ready.data, first_ready.events);
        assert_eq!(ready_data, 0xAB);
        assert_ne!(ready_events & linux_raw_sys::general::EPOLLIN, 0);

        assert_eq!(unsafe { libc::close(eventfd_descriptor) }, 0);
        assert_eq!(unsafe { libc::close(epoll_descriptor) }, 0);
    }
}
