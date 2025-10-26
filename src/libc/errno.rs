use std::io::Write;
use std::{cell::Cell, fmt::Display};

use crate::signature_matches_libc;
use libc;

#[repr(transparent)]
#[doc(alias = "errno")]
#[derive(PartialEq, Eq)]
pub struct Errno(pub(crate) u32);

#[no_mangle]
#[thread_local]
#[allow(non_upper_case_globals)]
pub static errno: Cell<Errno> = Cell::new(Errno(0));

pub fn set_errno(new_errno: Errno) {
    errno.set(new_errno);
}

impl Errno {
    pub const BADF: Self = Self(linux_raw_sys::errno::EBADF);
}

impl Into<u32> for &Errno {
    fn into(self) -> u32 {
        self.0
    }
}

impl Display for Errno {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Recognize errors documented in POSIX and use the documented strings.
        // <https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/errno.h.html>
        let message = match *self {
            Errno::BADF => "Bad file descriptor",
            ref unknown_errno => {
                return write!(f, "Unknown error: {}", Into::<u32>::into(unknown_errno))
            }
        };
        f.write_str(message)
    }
}

#[no_mangle]
unsafe extern "C" fn __errno_location() -> *mut Errno {
    signature_matches_libc!(core::mem::transmute(libc::__errno_location()));
    errno.as_ptr()
}

#[no_mangle]
unsafe extern "C" fn __xpg_strerror_r(errnum: Errno, buffer: *mut u8, length: usize) -> i32 {
    signature_matches_libc!(libc::strerror_r(
        core::mem::transmute(errnum),
        core::mem::transmute(buffer),
        length
    ));

    let mut cursor = std::io::Cursor::new(std::slice::from_raw_parts_mut(buffer, length));

    if write!(cursor, "{errnum}\0").is_err() {
        libc::ERANGE
    } else {
        0
    }
}
