use std::{
    alloc::{self, Layout},
    ptr,
};

use super::{with_stream_lock, IoFile, BUFFER_ALIGNMENT, EOF, STREAM_LIST_HEAD};
use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc, syscall,
    syscall::Syscall,
};

/// Unlinks a stream from the flush-all chain; flushing a closed stream would touch freed memory.
unsafe fn detach_stream(stream: *mut IoFile) {
    let mut cursor = *(&raw const STREAM_LIST_HEAD);
    if cursor.is_null() {
        return;
    }

    if cursor == stream {
        *(&raw mut STREAM_LIST_HEAD) = (*stream).chain;
        return;
    }

    while !(*cursor).chain.is_null() {
        if (*cursor).chain == stream {
            (*cursor).chain = (*stream).chain;
            return;
        }
        cursor = (*cursor).chain;
    }
}

unsafe fn release_buffer(stream: *mut IoFile) {
    let base = (*stream).buf_base;
    if base.is_null() {
        return;
    }

    let size = (*stream).buf_end.offset_from(base) as usize;
    alloc::dealloc(
        base,
        Layout::from_size_align_unchecked(size, BUFFER_ALIGNMENT),
    );
    (*stream).buf_base = ptr::null_mut();
    (*stream).buf_end = ptr::null_mut();
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn fclose(stream: *mut IoFile) -> i32 {
    signature_matches_libc!(libc::fclose(core::mem::transmute(stream)));

    if stream.is_null() {
        set_errno(Errno::BADF);
        return EOF;
    }

    let flush_result = with_stream_lock(stream, |file| file.flush_buffer());
    detach_stream(stream);
    let close_result = syscall!(Syscall::Close, (*stream).fileno);
    release_buffer(stream);

    if flush_result < 0 || close_result < 0 {
        EOF
    } else {
        0
    }
}
