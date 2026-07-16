use std::ffi::VaList;

use arbitrary_int::u12;
use bitbybit::{bitenum, bitfield};

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

const AT_FDCWD: isize = -100;

/// The permission bits of a file mode; the file-type nibble above them is not part of `open`'s mode argument.
#[bitfield(u32)]
struct FileMode {
    #[bits(0..=11, rw)]
    permissions: u12,
}

/// Both ABI names funnel here; a create-flavored call carries its permission `mode` in the variadic tail.
unsafe fn open_file(pathname: *const i8, flags: OFlags, mut args: VaList) -> i32 {
    let mode = if flags.create() || flags.create_unnamed_temporary_file() {
        FileMode::new_with_raw_value(args.arg::<u32>())
            .permissions()
            .value()
    } else {
        0
    };

    // Relative paths resolve against the CWD, so the dirfd is AT_FDCWD — not 0, which is stdin.
    let result = syscall!(Syscall::OpenAt, AT_FDCWD, pathname, flags.raw_value(), mode);

    if result < 0 {
        // The kernel returns the inverse of our errno...
        set_errno(Errno(result.abs() as u32));
        -1
    } else {
        result as i32
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn open64(pathname: *const i8, flags: OFlags, args: VaList) -> i32 {
    signature_matches_libc!(libc::open64(
        std::mem::transmute(pathname),
        std::mem::transmute(flags),
        args,
    ));
    open_file(pathname, flags, args)
}

// LFS alias: `open` is `open64` on x86_64 (O_LARGEFILE is a no-op).
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn open(pathname: *const i8, flags: OFlags, args: VaList) -> i32 {
    signature_matches_libc!(libc::open(
        std::mem::transmute(pathname),
        std::mem::transmute(flags),
        args,
    ));
    open_file(pathname, flags, args)
}

#[cfg_attr(not(test), no_mangle)]
pub static O_RDONLY: AccessMode = AccessMode::ReadOnly;
#[cfg_attr(not(test), no_mangle)]
pub static O_WRONLY: AccessMode = AccessMode::WriteOnly;
#[cfg_attr(not(test), no_mangle)]
pub static O_RDWR: AccessMode = AccessMode::ReadAndWrite;

#[bitenum(u2)]
pub enum AccessMode {
    ReadOnly = 0b00,
    WriteOnly = 0b01,
    ReadAndWrite = 0b10,
}

// TODO: clean up these value definitions...
#[cfg_attr(not(test), no_mangle)]
pub static O_CREAT: u32 = 64;
#[cfg_attr(not(test), no_mangle)]
pub static O_EXCL: u32 = 128;
#[cfg_attr(not(test), no_mangle)]
pub static O_NOCTTY: u32 = 256;
#[cfg_attr(not(test), no_mangle)]
pub static O_TRUNC: u32 = 512;
#[cfg_attr(not(test), no_mangle)]
pub static O_APPEND: u32 = 1024;
#[cfg_attr(not(test), no_mangle)]
pub static O_NONBLOCK: u32 = 2048;
#[cfg_attr(not(test), no_mangle)]
pub static O_DSYNC: u32 = 4096;
#[cfg_attr(not(test), no_mangle)]
pub static FASYNC: u32 = 8192;
#[cfg_attr(not(test), no_mangle)]
pub static O_DIRECT: u32 = 16384;
#[cfg_attr(not(test), no_mangle)]
pub static O_LARGEFILE: u32 = 32768;
#[cfg_attr(not(test), no_mangle)]
pub static O_DIRECTORY: u32 = 1 << 16;
#[cfg_attr(not(test), no_mangle)]
pub static O_NOFOLLOW: u32 = 131072;
#[cfg_attr(not(test), no_mangle)]
pub static O_NOATIME: u32 = 262144;
#[cfg_attr(not(test), no_mangle)]
pub static O_CLOEXEC: u32 = 524288;
#[cfg_attr(not(test), no_mangle)]
pub static __O_SYNC: u32 = 1048576;
#[cfg_attr(not(test), no_mangle)]
pub static O_SYNC: u32 = 1052672;
#[cfg_attr(not(test), no_mangle)]
pub static O_PATH: u32 = 2097152;
#[cfg_attr(not(test), no_mangle)]
pub static O_TMPFILE: u32 = 1 << 22 | O_DIRECTORY; // O_TMPFILE should always be passed with O_DIRECTORY
#[cfg_attr(not(test), no_mangle)]
pub static O_NDELAY: u32 = 2048;

#[bitfield(u32)]
pub struct OFlags {
    #[bits(0..=1, rw)]
    access_mode: Option<AccessMode>,
    #[bit(7, rw)]
    create: bool,
    #[bit(8, rw)]
    require_create: bool,
    #[bit(9, rw)]
    do_not_make_controlling_terminal: bool,
    #[bit(16, rw)]
    directory: bool,
    #[bit(18, rw)]
    do_not_follow_symbolic_link: bool,
    #[bit(19, rw)]
    close_on_exec: bool,
    #[bit(22, rw)]
    create_unnamed_temporary_file: bool,
}
