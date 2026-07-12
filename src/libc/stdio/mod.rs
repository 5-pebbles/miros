use std::{mem::offset_of, ptr};

use bitbybit::bitfield;

use crate::signature_matches_libc;

mod file;
mod file_lock;
mod stream_write;
mod writer;

use file_lock::FileLock;
pub(crate) use writer::vfprintf;

pub const EOF: i32 = -1;
pub const BUFFER_SIZE: usize = 8192;

/// glibc `_IO_FILE._flags`; miros interprets only these bits. `magic` (0xFBAD) fills the high half.
#[bitfield(u32)]
struct IoFlags {
    #[bit(1, rw)]
    unbuffered: bool,
    #[bit(3, rw)]
    no_writes: bool,
    #[bit(4, rw)]
    eof_seen: bool,
    #[bit(5, rw)]
    err_seen: bool,
    #[bit(9, rw)]
    line_buffered: bool,
    #[bit(11, rw)]
    currently_putting: bool,
    #[bits(16..=31, rw)]
    magic: u16,
}

const IO_MAGIC: u16 = 0xFBAD;

// SAFETY: glibc froze these `_IO_FILE` offsets in the libio rewrite (2.1, 1999); moving would break every binary ever linked against glibc, so hardcoding them is probably safe.
const _: () = {
    assert!(offset_of!(IoFile, flags) == 0);
    assert!(offset_of!(IoFile, read_ptr) == 8);
    assert!(offset_of!(IoFile, read_end) == 16);
    assert!(offset_of!(IoFile, write_ptr) == 40);
    assert!(offset_of!(IoFile, write_end) == 48);
};
/// Offsets 0/8/16/40/48 are pinned to glibc `struct _IO_FILE` — inlined `-O2`
/// `putc_unlocked`/`getc_unlocked`/`feof_unlocked` read them there.
#[repr(C)]
pub struct IoFile {
    flags: IoFlags,
    read_ptr: *mut u8,
    read_end: *mut u8,
    read_base: *mut u8,
    write_base: *mut u8,
    write_ptr: *mut u8,
    write_end: *mut u8,
    buf_base: *mut u8,
    buf_end: *mut u8,
    fileno: i32,
    chain: *mut IoFile,
    lock: FileLock,
}

unsafe impl Send for IoFile {}
unsafe impl Sync for IoFile {}

impl IoFile {
    const fn new(fileno: i32, flags: IoFlags, chain: *mut IoFile) -> Self {
        Self {
            flags,
            read_ptr: ptr::null_mut(),
            read_end: ptr::null_mut(),
            read_base: ptr::null_mut(),
            write_base: ptr::null_mut(),
            write_ptr: ptr::null_mut(),
            write_end: ptr::null_mut(),
            buf_base: ptr::null_mut(),
            buf_end: ptr::null_mut(),
            fileno,
            chain,
            lock: FileLock::new(),
        }
    }
}

const fn base_flags() -> IoFlags {
    IoFlags::new_with_raw_value(0).with_magic(IO_MAGIC)
}

// `stdin`/`stdout`/`stderr` are the exported `IoFile *` pointers; these are the objects they point at.
static mut STDIN_FILE: IoFile = IoFile::new(0, base_flags().with_no_writes(true), ptr::null_mut());
static mut STDOUT_FILE: IoFile = IoFile::new(
    1,
    base_flags().with_line_buffered(true),
    &raw mut STDERR_FILE,
);
static mut STDERR_FILE: IoFile =
    IoFile::new(2, base_flags().with_unbuffered(true), &raw mut STDIN_FILE);

static mut STREAM_LIST_HEAD: *mut IoFile = &raw mut STDOUT_FILE;

#[cfg_attr(not(test), no_mangle)]
#[allow(non_upper_case_globals)]
static mut stdin: *mut IoFile = &raw mut STDIN_FILE;

#[cfg_attr(not(test), no_mangle)]
#[allow(non_upper_case_globals)]
static mut stdout: *mut IoFile = &raw mut STDOUT_FILE;

#[cfg_attr(not(test), no_mangle)]
#[allow(non_upper_case_globals)]
static mut stderr: *mut IoFile = &raw mut STDERR_FILE;

pub(crate) unsafe fn stdout_ptr() -> *mut IoFile {
    *(&raw const stdout)
}

/// The one place stdio locking lives; the `*_unlocked` API and inlined fast paths bypass it.
pub(super) unsafe fn with_stream_lock<R>(
    stream: *mut IoFile,
    action: impl FnOnce(&mut IoFile) -> R,
) -> R {
    (*stream).lock.lock();
    let result = action(&mut *stream);
    (*stream).lock.unlock();
    result
}

pub(crate) unsafe fn flush_all_streams() {
    let mut stream = *(&raw const STREAM_LIST_HEAD);
    while !stream.is_null() {
        with_stream_lock(stream, |file| unsafe { file.flush_buffer() });
        stream = (*stream).chain;
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn fflush(stream: *mut IoFile) -> i32 {
    signature_matches_libc!(libc::fflush(core::mem::transmute(stream)));

    if stream.is_null() {
        flush_all_streams();
        return 0;
    }

    with_stream_lock(stream, |file| unsafe { file.flush_buffer() })
}
