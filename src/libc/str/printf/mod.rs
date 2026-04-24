mod format;
mod parse;
mod specifier;

use std::{
    ffi::VaList,
    fs::File,
    io::{self, stdout, BufWriter, Write},
    mem::ManuallyDrop,
    os::fd::{AsRawFd, FromRawFd},
};

use format::Formatter;
use parse::PrintfItem;
use specifier::ResolvedSpecifier;

/// Writes bytes sequentially to a raw pointer with no bounds checking.
/// Used by `sprintf` / `vsprintf` to write into caller-provided buffers.
struct UncheckedBufWriter {
    cursor: *mut u8,
}

impl UncheckedBufWriter {
    fn new(destination: *mut i8) -> Self {
        Self {
            cursor: destination as *mut u8,
        }
    }
}

impl Write for UncheckedBufWriter {
    fn write(&mut self, source: &[u8]) -> io::Result<usize> {
        unsafe {
            core::ptr::copy_nonoverlapping(source.as_ptr(), self.cursor, source.len());
            self.cursor = self.cursor.add(source.len());
        }
        Ok(source.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn printf(format: *const i8, args: ...) -> i32 {
    vdprintf(stdout().as_raw_fd(), format, args)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn sprintf(destination: *mut i8, format: *const i8, args: ...) -> i32 {
    vsprintf(destination, format, args)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn vsprintf(
    destination: *mut i8,
    format: *const i8,
    mut args: VaList<'_>,
) -> i32 {
    let writer = UncheckedBufWriter::new(destination);
    let mut formatter = Formatter::new(writer);

    for item in parse::PrintfParser::new(format) {
        match item {
            PrintfItem::Literal(bytes) => formatter.write_bytes(bytes),
            PrintfItem::Specifier(spec) => {
                let resolved = ResolvedSpecifier::from_parsed(spec, &mut args);
                formatter.format(&resolved, &mut args);
            }
        }
    }

    let bytes_written = formatter.finish();
    // WARN: Null-terminate even on error — glibc does this, and callers may read the buffer regardless of the return value.
    *destination.add(bytes_written.max(0) as usize) = 0;
    bytes_written
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn vdprintf(
    file_descriptor: i32,
    format: *const i8,
    mut args: VaList<'_>,
) -> i32 {
    let file = ManuallyDrop::new(File::from_raw_fd(file_descriptor));
    let writer = BufWriter::new(&*file);
    let mut formatter = Formatter::new(writer);

    for item in parse::PrintfParser::new(format) {
        match item {
            PrintfItem::Literal(bytes) => formatter.write_bytes(bytes),
            PrintfItem::Specifier(spec) => {
                let resolved = ResolvedSpecifier::from_parsed(spec, &mut args);
                formatter.format(&resolved, &mut args);
            }
        }
    }

    formatter.finish()
}

#[cfg(test)]
#[allow(improper_ctypes_definitions)]
mod tests {
    use std::ffi::CString;

    use super::*;

    unsafe extern "C" fn test_sprintf(format: *const i8, args: ...) -> Vec<u8> {
        let mut buffer = [0u8; 4096];
        let bytes_written = vsprintf(buffer.as_mut_ptr() as *mut i8, format, args);
        buffer[..bytes_written as usize].to_vec()
    }

    macro_rules! sprintf_tests {
        (mod $mod_name:ident {
            $($name:ident, $format:expr, ($($arg:expr),*), $expected:expr);* $(;)?
        }) => {
            mod $mod_name {
                use super::*;
                $(
                    #[test]
                    fn $name() {
                        let format =
                            CString::new($format).expect("format should contain no internal nulls");
                        let output = unsafe { test_sprintf(format.as_ptr(), $($arg),*) };
                        assert_eq!(output, $expected.as_bytes());
                    }
                )*
            }
        };
    }

    sprintf_tests!(mod end_to_end {
        pure_literal,        "hello",                    (),                                     "hello";
        percent_escape,      "100%%",                    (),                                     "100%";
        signed_int,          "%d",                       (42i32),                                "42";
        negative_int,        "%d",                       (-42i32),                               "-42";
        multiple_specifiers, "%d + %d = %d",             (1i32, 2i32, 3i32),                     "1 + 2 = 3";
        star_width,          "%*d",                      (10i32, 42i32),                         "        42";
        star_precision,      "%.*d",                     (5i32, 42i32),                          "00042";
        string,              "%s",                       (c"hello".as_ptr()),                    "hello";
        char_format,         "%c",                       (65i32),                                "A";
        null_pointer,        "%p",                       (core::ptr::null::<()>()),              "(nil)";
        hex_alternate,       "%#x",                      (255u32),                               "0xff";
        padded_string,       "[%-10s]",                  (c"hi".as_ptr()),                       "[hi        ]";
        zero_padded_int,     "%08d",                     (42i32),                                "00000042";
        forced_sign,         "%+d",                      (42i32),                                "+42";
        literal_and_specs,   "val=%d hex=%#x",           (42i32, 255u32),                        "val=42 hex=0xff";
    });
}
