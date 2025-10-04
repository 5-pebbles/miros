use std::{arch::asm, ffi::c_char};

use bitbybit::{bitenum, bitfield};

#[no_mangle]
pub static O_RDONLY: AccessMode = AccessMode::ReadOnly;
#[no_mangle]
pub static O_WRONLY: AccessMode = AccessMode::WriteOnly;
#[no_mangle]
pub static O_RDWR: AccessMode = AccessMode::ReadAndWrite;

#[bitenum(u2)]
pub enum AccessMode {
    ReadOnly = 0b00,
    WriteOnly = 0b01,
    ReadAndWrite = 0b10,
}

// TODO: clean up these value definitions...
#[no_mangle]
pub static O_CREAT: u32 = 64;
#[no_mangle]
pub static O_EXCL: u32 = 128;
#[no_mangle]
pub static O_NOCTTY: u32 = 256;
#[no_mangle]
pub static O_TRUNC: u32 = 512;
#[no_mangle]
pub static O_APPEND: u32 = 1024;
#[no_mangle]
pub static O_NONBLOCK: u32 = 2048;
#[no_mangle]
pub static O_DSYNC: u32 = 4096;
#[no_mangle]
pub static FASYNC: u32 = 8192;
#[no_mangle]
pub static O_DIRECT: u32 = 16384;
#[no_mangle]
pub static O_LARGEFILE: u32 = 32768;
#[no_mangle]
pub static O_DIRECTORY: u32 = 1 << 16;
#[no_mangle]
pub static O_NOFOLLOW: u32 = 131072;
#[no_mangle]
pub static O_NOATIME: u32 = 262144;
#[no_mangle]
pub static O_CLOEXEC: u32 = 524288;
#[no_mangle]
pub static __O_SYNC: u32 = 1048576;
#[no_mangle]
pub static O_SYNC: u32 = 1052672;
#[no_mangle]
pub static O_PATH: u32 = 2097152;
#[no_mangle]
pub static O_TMPFILE: u32 = 1 << 22 | O_DIRECTORY; // O_TMPFILE should always be passed with O_DIRECTORY
#[no_mangle]
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

pub unsafe fn openat(
    directory_file_descriptor: Option<isize>,
    pathname: *const c_char,
    flags: OFlags,
    mode: u32,
) -> i32 {
    const OPENAT: usize = 257;

    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") OPENAT => result,
        in("rdi") directory_file_descriptor.unwrap_or_default(),
        in("rsi") pathname,
        in("rdx") flags.raw_value(),
        in("r10") mode,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags)
    );
    result as i32
}
