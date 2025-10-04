use crate::{
    signature_matches_libc,
    syscall::openat::{openat, OFlags},
};
use std::ffi::VaList;

const AT_FDCWD: isize = -100;
pub const S_IFMT: u32 = 1111 << 12;

#[no_mangle]
unsafe extern "C" fn open64(pathname: *const i8, flags: OFlags, mut args: VaList) -> i32 {
    signature_matches_libc!(libc::open64(
        core::mem::transmute(pathname),
        core::mem::transmute(flags),
        args,
    ));

    let mode = if flags.create() || flags.create_unnamed_temporary_file() {
        args.arg::<u32>() & !S_IFMT
    } else {
        0
    };

    openat(Some(AT_FDCWD), pathname, flags, mode)
}
