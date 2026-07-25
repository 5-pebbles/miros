use std::{
    alloc,
    mem::MaybeUninit,
    os::fd::{AsRawFd, BorrowedFd},
    ptr,
};

use super::{IoFile, BUFFER_SIZE, EOF};
use crate::{
    libc::{
        errno::Errno,
        fs::{fstat::FileStatus, isatty::file_descriptor_isatty},
    },
    syscall::{syscall, Syscall},
};

impl IoFile {
    /// Idempotent: picks buffering mode (glibc's line-buffered iff a tty) and allocates once.
    unsafe fn ensure_buffer(&mut self) {
        if !self.buf_base.is_null() {
            return;
        }

        let file_descriptor = BorrowedFd::borrow_raw(self.fileno);

        if !self.flags.unbuffered() {
            self.flags = self
                .flags
                .with_line_buffered(file_descriptor_isatty(file_descriptor));
        }

        let size = optimal_buffer_size(file_descriptor);
        let layout = alloc::Layout::from_size_align(size, 16).unwrap();
        let buffer = alloc::alloc(layout);
        if buffer.is_null() {
            alloc::handle_alloc_error(layout);
        }
        self.buf_base = buffer;
        self.buf_end = buffer.add(size);
    }

    unsafe fn begin_write(&mut self) {
        if self.flags.currently_putting() && !self.buf_base.is_null() {
            return;
        }
        self.ensure_buffer();
        self.write_base = self.buf_base;
        self.write_ptr = self.buf_base;
        self.write_end = if self.flags.line_buffered() || self.flags.unbuffered() {
            self.buf_base
        } else {
            self.buf_end
        };
        self.flags = self.flags.with_currently_putting(true);
    }

    /// Sets `_IO_ERR_SEEN` and returns -1 on write failure.
    pub(super) unsafe fn flush_buffer(&mut self) -> i32 {
        let mut cursor = self.write_base;
        while cursor < self.write_ptr {
            let remaining = self.write_ptr.offset_from(cursor) as usize;
            let written = syscall!(Syscall::Write, self.fileno, cursor, remaining);
            if written < 0 {
                if written == -(Errno::INTR.into_raw() as isize) {
                    continue;
                }
                self.flags = self.flags.with_err_seen(true);
                return -1;
            }
            cursor = cursor.add(written as usize);
        }
        self.write_ptr = self.write_base;
        0
    }

    /// False on write error.
    unsafe fn ensure_write_space(&mut self) -> bool {
        self.write_ptr < self.buf_end || self.flush_buffer() >= 0
    }

    /// Append one byte, flushing first if the buffer is full. `c == EOF` is glibc's flush request.
    pub(super) unsafe fn overflow(&mut self, c: i32) -> i32 {
        self.begin_write();

        if c == EOF {
            return if self.flush_buffer() < 0 { EOF } else { 0 };
        }

        if !self.ensure_write_space() {
            return EOF;
        }
        *self.write_ptr = c as u8;
        self.write_ptr = self.write_ptr.add(1);

        if self.flags.unbuffered() || (self.flags.line_buffered() && c as u8 == b'\n') {
            if self.flush_buffer() < 0 {
                return EOF;
            }
        }
        c & 0xff
    }

    /// Returns bytes accepted — short of `bytes.len()` only on write error.
    pub(super) unsafe fn write_bytes(&mut self, bytes: &[u8]) -> usize {
        self.begin_write();

        let mut remaining = bytes;
        while !remaining.is_empty() {
            if !self.ensure_write_space() {
                break;
            }
            let room = self.buf_end.offset_from(self.write_ptr) as usize;
            let take = room.min(remaining.len());
            ptr::copy_nonoverlapping(remaining.as_ptr(), self.write_ptr, take);
            self.write_ptr = self.write_ptr.add(take);
            let chunk = &remaining[..take];
            remaining = &remaining[take..];

            // Line-buffered flush is whole-buffer, not up to the last newline — more eager than glibc, still POSIX-legal.
            if self.flags.unbuffered() || (self.flags.line_buffered() && chunk.contains(&b'\n')) {
                if self.flush_buffer() < 0 {
                    break;
                }
            }
        }
        bytes.len() - remaining.len()
    }
}

fn optimal_buffer_size(file_descriptor: BorrowedFd<'_>) -> usize {
    let mut status = MaybeUninit::<FileStatus>::uninit();
    let result = unsafe {
        syscall!(
            Syscall::FStat,
            file_descriptor.as_raw_fd(),
            status.as_mut_ptr()
        )
    };
    if result == 0 {
        let block_size = unsafe { status.assume_init() }.block_size;
        if block_size > 0 {
            return block_size as usize;
        }
    }
    BUFFER_SIZE
}

/// The extern fallback glibc's inlined `putc_unlocked` calls. Caller owns the lock.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn __overflow(stream: *mut IoFile, c: i32) -> i32 {
    (*stream).overflow(c)
}
