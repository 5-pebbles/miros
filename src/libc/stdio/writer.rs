use std::{
    ffi::VaList,
    io::{self, Write},
};

use super::{stdout_ptr, with_stream_lock, IoFile};
use crate::{libc::str::printf::format_into, signature_matches_libc};

/// Adapts the printf `Formatter` onto an `IoFile`; `flush` is a no-op since stdio flushing is separate.
struct FileWriter<'a>(&'a mut IoFile);

impl Write for FileWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        Ok(unsafe { self.0.write_bytes(bytes) })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg_attr(not(test), no_mangle)]
pub(crate) unsafe extern "C" fn vfprintf(
    stream: *mut IoFile,
    format: *const i8,
    args: VaList<'_>,
) -> i32 {
    with_stream_lock(stream, |file| unsafe { format_into(FileWriter(file), format, args) })
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn fprintf(stream: *mut IoFile, format: *const i8, args: ...) -> i32 {
    signature_matches_libc!(libc::fprintf(core::mem::transmute(stream), format, args));
    vfprintf(stream, format, args)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn vprintf(format: *const i8, args: VaList<'_>) -> i32 {
    vfprintf(stdout_ptr(), format, args)
}
