use std::{ffi::CStr, os::raw::c_void, slice};

use super::{stdout_ptr, with_stream_lock, IoFile, EOF};
use crate::signature_matches_libc;

/// Unlocked single-byte put.
unsafe fn put_byte(file: &mut IoFile, byte: i32) -> i32 {
    if file.write_ptr < file.write_end {
        *file.write_ptr = byte as u8;
        file.write_ptr = file.write_ptr.add(1);
        byte & 0xff
    } else {
        file.overflow(byte)
    }
}

/// Locked/unlocked bulk write shared by `fputs`/`fwrite`.
unsafe fn write_stream(stream: *mut IoFile, bytes: &[u8], lock: bool) -> usize {
    if lock {
        with_stream_lock(stream, |file| unsafe { file.write_bytes(bytes) })
    } else {
        (*stream).write_bytes(bytes)
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn fputc(character: i32, stream: *mut IoFile) -> i32 {
    signature_matches_libc!(libc::fputc(character, core::mem::transmute(stream)));
    with_stream_lock(stream, |file| unsafe { put_byte(file, character) })
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn putc(character: i32, stream: *mut IoFile) -> i32 {
    // `libc` has no `putc` (a C macro), but `putc`/`fputc` share the glibc signature `fputc` checks.
    fputc(character, stream)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn putchar(character: i32) -> i32 {
    signature_matches_libc!(libc::putchar(character));
    fputc(character, stdout_ptr())
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn fputc_unlocked(character: i32, stream: *mut IoFile) -> i32 {
    put_byte(&mut *stream, character)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn putc_unlocked(character: i32, stream: *mut IoFile) -> i32 {
    fputc_unlocked(character, stream)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn putchar_unlocked(character: i32) -> i32 {
    fputc_unlocked(character, stdout_ptr())
}

unsafe fn fputs_common(string: *const i8, stream: *mut IoFile, lock: bool) -> i32 {
    let bytes = CStr::from_ptr(string).to_bytes();
    if write_stream(stream, bytes, lock) == bytes.len() {
        bytes.len() as i32
    } else {
        EOF
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn fputs(string: *const i8, stream: *mut IoFile) -> i32 {
    signature_matches_libc!(libc::fputs(string, core::mem::transmute(stream)));
    fputs_common(string, stream, true)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn fputs_unlocked(string: *const i8, stream: *mut IoFile) -> i32 {
    fputs_common(string, stream, false)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn puts(string: *const i8) -> i32 {
    signature_matches_libc!(libc::puts(string));
    let bytes = CStr::from_ptr(string).to_bytes();

    // String and its newline go out under one lock, so concurrent writers can't interleave.
    let (body, newline) = with_stream_lock(stdout_ptr(), |file| unsafe {
        (file.write_bytes(bytes), file.write_bytes(b"\n"))
    });

    if body == bytes.len() && newline == 1 {
        (bytes.len() + 1) as i32
    } else {
        EOF
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn fwrite(
    pointer: *const c_void,
    size: usize,
    count: usize,
    stream: *mut IoFile,
) -> usize {
    signature_matches_libc!(libc::fwrite(
        pointer,
        size,
        count,
        core::mem::transmute(stream)
    ));
    fwrite_common(pointer, size, count, stream, true)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn fwrite_unlocked(
    pointer: *const c_void,
    size: usize,
    count: usize,
    stream: *mut IoFile,
) -> usize {
    fwrite_common(pointer, size, count, stream, false)
}

unsafe fn fwrite_common(
    pointer: *const c_void,
    size: usize,
    count: usize,
    stream: *mut IoFile,
    lock: bool,
) -> usize {
    let Some(total) = size.checked_mul(count) else {
        return 0;
    };
    if total == 0 {
        return 0;
    }

    let bytes = slice::from_raw_parts(pointer as *const u8, total);
    // fwrite reports whole items, not bytes.
    write_stream(stream, bytes, lock) / size
}
