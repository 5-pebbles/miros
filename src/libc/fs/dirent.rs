use std::{
    alloc::{alloc, dealloc, Layout},
    ffi::c_char,
    os::fd::RawFd,
    ptr,
};

use super::open::OFlags;
use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

const AT_FDCWD: isize = -100;
const DIRECTORY_BUFFER_SIZE: usize = 1 << 15;

/// Our `DIR`: the directory fd plus a `getdents64` staging buffer and a cursor into it. Callers only ever hold an opaque `*mut DIR`, so the layout is ours; the buffer trails the header so it stays 8-aligned for the kernel's entries.
#[repr(C)]
pub struct DirectoryStream {
    file_descriptor: RawFd,
    buffer_filled: usize,
    buffer_cursor: usize,
    buffer: [u8; DIRECTORY_BUFFER_SIZE],
}

/// The kernel's `linux_dirent64`; its header is byte-identical to glibc's `struct dirent`, so `readdir` returns a pointer straight into the buffer. A NUL-terminated name follows inline.
#[repr(C)]
struct KernelDirent {
    inode: u64,
    offset: i64,
    record_length: u16,
    file_type: u8,
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn opendir(name: *const c_char) -> *mut DirectoryStream {
    signature_matches_libc!(std::mem::transmute(libc::opendir(name)));

    let flags = OFlags::ZERO.with_directory(true).with_close_on_exec(true);
    let file_descriptor = syscall!(Syscall::OpenAt, AT_FDCWD, name, flags.raw_value(), 0usize);
    if file_descriptor < 0 {
        set_errno(Errno(file_descriptor.abs() as u32));
        return ptr::null_mut();
    }

    let stream = alloc(Layout::new::<DirectoryStream>()) as *mut DirectoryStream;
    if stream.is_null() {
        syscall!(Syscall::Close, file_descriptor);
        set_errno(Errno::NOMEM);
        return ptr::null_mut();
    }
    (*stream).file_descriptor = file_descriptor as RawFd;
    (*stream).buffer_filled = 0;
    (*stream).buffer_cursor = 0;
    stream
}

/// The next entry, refilling from `getdents64` when the buffer drains. NULL with errno untouched is end-of-directory; NULL with errno set is an error — the caller clears errno first to tell them apart.
unsafe fn next_entry(stream: *mut DirectoryStream) -> *mut KernelDirent {
    let stream = &mut *stream;
    if stream.buffer_cursor >= stream.buffer_filled {
        let filled = syscall!(
            Syscall::GetDents64,
            stream.file_descriptor,
            stream.buffer.as_mut_ptr(),
            DIRECTORY_BUFFER_SIZE
        );
        if filled < 0 {
            set_errno(Errno(filled.abs() as u32));
            return ptr::null_mut();
        }
        if filled == 0 {
            return ptr::null_mut();
        }
        stream.buffer_filled = filled as usize;
        stream.buffer_cursor = 0;
    }

    let entry = stream.buffer.as_mut_ptr().add(stream.buffer_cursor) as *mut KernelDirent;
    stream.buffer_cursor += (*entry).record_length as usize;
    entry
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn readdir(stream: *mut DirectoryStream) -> *mut libc::dirent {
    signature_matches_libc!(libc::readdir(std::mem::transmute(stream)));
    next_entry(stream).cast()
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn readdir64(stream: *mut DirectoryStream) -> *mut libc::dirent64 {
    signature_matches_libc!(libc::readdir64(std::mem::transmute(stream)));
    next_entry(stream).cast()
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn closedir(stream: *mut DirectoryStream) -> i32 {
    signature_matches_libc!(libc::closedir(std::mem::transmute(stream)));
    if stream.is_null() {
        set_errno(Errno::INVAL);
        return -1;
    }
    let file_descriptor = (*stream).file_descriptor;
    dealloc(stream as *mut u8, Layout::new::<DirectoryStream>());
    let result = syscall!(Syscall::Close, file_descriptor);
    if result < 0 {
        set_errno(Errno(result.abs() as u32));
        -1
    } else {
        0
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn dirfd(stream: *mut DirectoryStream) -> i32 {
    signature_matches_libc!(libc::dirfd(std::mem::transmute(stream)));
    (*stream).file_descriptor
}
