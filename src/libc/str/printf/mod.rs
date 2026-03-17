mod format;
mod parse;
mod specifier;

use std::{ffi::VaList, fs::File, io::BufWriter, mem::ManuallyDrop, os::fd::FromRawFd};

use format::Formatter;
use parse::PrintfItem;
use specifier::ResolvedSpecifier;

const STDOUT_FD: i32 = 1;

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn printf(format: *const i8, mut args: ...) -> i32 {
    write_formatted_to_file_descriptor(STDOUT_FD, format, args.as_va_list())
}

unsafe fn write_formatted_to_file_descriptor(
    file_descriptor: i32,
    format: *const i8,
    mut args: VaList<'_, '_>,
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

    unsafe extern "C" fn sprintf(format: *const i8, mut args: ...) -> Vec<u8> {
        let mut output = Vec::new();
        {
            let mut formatter = Formatter::new(&mut output);
            for item in parse::PrintfParser::new(format) {
                match item {
                    PrintfItem::Literal(bytes) => formatter.write_bytes(bytes),
                    PrintfItem::Specifier(spec) => {
                        let resolved = ResolvedSpecifier::from_parsed(spec, &mut args.as_va_list());
                        formatter.format(&resolved, &mut args.as_va_list());
                    }
                }
            }
        }
        output
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
                        let output = unsafe { sprintf(format.as_ptr(), $($arg),*) };
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
        string,              "%s",                       (b"hello\0".as_ptr() as *const i8),     "hello";
        char_format,         "%c",                       (65i32),                                "A";
        null_pointer,        "%p",                       (core::ptr::null::<()>()),              "(nil)";
        hex_alternate,       "%#x",                      (255u32),                               "0xff";
        padded_string,       "[%-10s]",                  (b"hi\0".as_ptr() as *const i8),        "[hi        ]";
        zero_padded_int,     "%08d",                     (42i32),                                "00000042";
        forced_sign,         "%+d",                      (42i32),                                "+42";
        literal_and_specs,   "val=%d hex=%#x",           (42i32, 255u32),                        "val=42 hex=0xff";
    });
}
