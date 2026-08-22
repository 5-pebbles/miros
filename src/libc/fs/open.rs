use std::ffi::VaList;

use arbitrary_int::{u12, u3};
use bitbybit::{bitenum, bitfield};

use crate::{libc::translate_syscall_result, signature_matches_libc, syscall, syscall::Syscall};

const AT_FDCWD: isize = -100;

#[bitfield(u3)]
struct UnixPermissionClass {
    #[bit(0, rw)]
    exec: bool,
    #[bit(1, rw)]
    write: bool,
    #[bit(2, rw)]
    read: bool,
}

#[bitfield(u12)]
struct UnixPermissions {
    #[bits(0..=2, rw)]
    other: UnixPermissionClass,
    #[bits(3..=5, rw)]
    group: UnixPermissionClass,
    #[bits(6..=8, rw)]
    owner: UnixPermissionClass,
    #[bit(9, rw)]
    sticky: bool,
    #[bit(10, rw)]
    set_group_id: bool,
    #[bit(11, rw)]
    set_user_id: bool,
}

#[bitenum(u4)]
pub enum UnixFileType {
    NamedPipe = 0b0001,
    CharacterDevice = 0b0010,
    Directory = 0b0100,
    BlockDevice = 0b0110,
    RegularFile = 0b1000,
    SymbolicLink = 0b1010,
    Socket = 0b1100,
}

#[bitfield(u32)]
struct UnixFileMode {
    #[bits(0..=11, rw)]
    permissions: u12,
    #[bits(12..=15, rw)]
    file_type: Option<UnixFileType>,
}

unsafe fn open_file(pathname: *const i8, flags: OFlags, mut args: VaList) -> i32 {
    let mode = if flags.create() || flags.create_unnamed_temporary_file() {
        UnixFileMode::new_with_raw_value(args.next_arg::<u32>())
            .permissions()
            .value()
    } else {
        UnixPermissions::ZERO.raw_value().value()
    };

    // Relative paths resolve against the CWD, so the dirfd is AT_FDCWD.
    let result = syscall!(Syscall::OpenAt, AT_FDCWD, pathname, flags.raw_value(), mode);
    translate_syscall_result(result) as i32
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

// LFS alias: `open` is `open64` on x86_64, where O_LARGEFILE is a no-op.
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
    #[bit(18, rw)]
    do_not_follow_symbolic_link: bool,
    #[bit(22, rw)]
    create_unnamed_temporary_file: bool,
}
