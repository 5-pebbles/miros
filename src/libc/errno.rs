use std::cell::Cell;

#[repr(transparent)]
#[doc(alias = "errno")]
pub struct Errno(pub(crate) u32);

thread_local! {
    #[no_mangle]
    #[allow(non_upper_case_globals)]
    pub static errno: Cell<Errno> = Cell::new(Errno(0));
}

pub fn set_errno(new_errno: Errno) {
    errno.with(|e| e.set(new_errno));
}

impl Errno {
    pub const BADF: Self = Self(linux_raw_sys::errno::EBADF);
}

// #[no_mangle]
// unsafe extern "C" fn __xpg_strerror_r(errnum: Errno, buf: *mut c_char, buflen: usize) -> u32 {
//     0
// }
