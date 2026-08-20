use std::ffi::{c_int, c_uint};

use crate::libc::net::net_syscall_pass_through;

net_syscall_pass_through! {
    fn eventfd(initial_value: c_uint, flags: c_int) -> c_int = Syscall::EventFd2;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eventfd_accumulates_writes_and_drains_on_read() {
        let descriptor = unsafe { eventfd(0, 0) };
        assert_ne!(descriptor, -1);

        let one: u64 = 1;
        let two: u64 = 2;
        assert_eq!(
            unsafe { libc::write(descriptor, (&one as *const u64).cast(), 8) },
            8
        );
        assert_eq!(
            unsafe { libc::write(descriptor, (&two as *const u64).cast(), 8) },
            8
        );

        let mut counter: u64 = 0;
        assert_eq!(
            unsafe { libc::read(descriptor, (&mut counter as *mut u64).cast(), 8) },
            8
        );
        assert_eq!(counter, 3);
        assert_eq!(unsafe { libc::close(descriptor) }, 0);
    }
}
