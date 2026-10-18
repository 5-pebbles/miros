use super::IoFile;

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

    if !file.read_ptr.is_null() {
        file.read_ptr = file.read_end;
    }

    if !file.write_base.is_null() {
        file.write_ptr = file.write_base;
    }
    file.flags = file.flags.with_currently_putting(false);
}
