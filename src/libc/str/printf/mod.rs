mod format;
mod parse;
mod specifier;

use std::{ffi::VaList, fs::File, io::BufWriter, mem::ManuallyDrop, os::fd::FromRawFd};

use format::Formatter;
use parse::PrintfItem;
use specifier::ResolvedSpecifier;

const STDOUT_FD: i32 = 1;

#[no_mangle]
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
