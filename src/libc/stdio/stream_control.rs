use super::IoFile;
use crate::{syscall, syscall::Syscall};

const SEEK_CUR: i32 = 1;

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn clearerr_unlocked(stream: *mut IoFile) {
    (*stream).flags = (*stream).flags.with_eof_seen(false).with_err_seen(false);
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn __fpending(stream: *mut IoFile) -> usize {
    let file = &*stream;
    if file.write_base.is_null() || file.write_ptr.is_null() {
        return 0;
    }

    file.write_ptr.offset_from(file.write_base) as usize
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn __fpurge(stream: *mut IoFile) {
    let file = &mut *stream;

    // Input is discarded by seeking back, so the bytes stay readable from the descriptor.
    if !file.read_ptr.is_null() && !file.read_end.is_null() {
        let unread = file.read_end.offset_from(file.read_ptr);
        if unread > 0 {
            syscall!(Syscall::LSeek, file.fileno, -unread, SEEK_CUR);
        }
        file.read_ptr = file.read_end;
    }

    if !file.write_base.is_null() {
        file.write_ptr = file.write_base;
    }
    file.flags = file.flags.with_currently_putting(false);
}
