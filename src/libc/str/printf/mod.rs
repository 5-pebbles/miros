mod format;
mod parse;

use std::{ffi::VaList, fs::File, io::BufWriter, mem::ManuallyDrop, os::fd::FromRawFd};

use format::Formatter;
use parse::{
    Conversion, DimensionSpecifier, LengthModifier, PadMode, PrintfItem, PrintfParser,
    PrintfSpecifier, SignMode,
};

const STDOUT_FD: i32 = 1;

#[no_mangle]
unsafe extern "C" fn printf(format: *const i8, mut args: ...) -> i32 {
    fmt_to_file_descriptor(STDOUT_FD, format, args.as_va_list())
}

unsafe fn fmt_to_file_descriptor(
    file_descriptor: i32,
    format: *const i8,
    mut args: VaList<'_, '_>,
) -> i32 {
    let file = ManuallyDrop::new(File::from_raw_fd(file_descriptor));
    let writer = BufWriter::new(&*file);
    let mut formatter = Formatter::new(writer);

    for item in PrintfParser::new(format) {
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

#[derive(Clone, Copy)]
pub(super) struct ResolvedSpecifier {
    pub sign_mode: SignMode,
    pub pad_mode: PadMode,
    pub alternate: bool,
    pub width: Option<usize>,
    pub precision: Option<usize>,
    pub length: LengthModifier,
    pub conversion: Conversion,
}

impl ResolvedSpecifier {
    /// Resolve `*` width/precision from the argument list and apply all
    /// flag-interaction rules (C11 §7.21.6.1p6).
    unsafe fn from_parsed(spec: PrintfSpecifier, args: &mut VaList<'_, '_>) -> Self {
        let mut left_align = spec.flags.left_justify;

        let width = match spec.width {
            DimensionSpecifier::Unspecified => None,
            DimensionSpecifier::Fixed(value) => Some(value),
            DimensionSpecifier::FromNextArg => match args.arg::<i32>() {
                raw if raw < 0 => {
                    left_align = true;
                    Some(raw.unsigned_abs() as usize)
                }
                raw => Some(raw as usize),
            },
        };

        let precision = match spec.precision {
            DimensionSpecifier::Unspecified => None,
            DimensionSpecifier::Fixed(value) => Some(value),
            DimensionSpecifier::FromNextArg => {
                let raw = args.arg::<i32>();
                (raw >= 0).then(|| raw as usize)
            }
        };

        // Sign mode: only meaningful for signed conversions (integers and floats).
        // `+` overrides ` ` (C11 §7.21.6.1p6).
        let sign_mode = if !spec.conversion.is_signed() {
            SignMode::None
        } else if spec.flags.force_sign {
            SignMode::Force
        } else if spec.flags.space_sign {
            SignMode::Space
        } else {
            SignMode::None
        };

        // `0` flag: only valid for numeric conversions.
        // `-` overrides `0` (C11 §7.21.6.1p6).
        // Precision suppresses `0` for integer conversions (C11 §7.21.6.1p6).
        // Must happen after resolving `.*` because a negative argument means
        // "precision omitted", which should NOT suppress zero-padding.
        let zero_pad = spec.flags.zero_pad
            && spec.conversion.is_numeric()
            && !left_align
            && !(precision.is_some() && spec.conversion.is_integer());

        let pad_mode = if left_align {
            PadMode::LeftAlign
        } else if zero_pad {
            PadMode::ZeroPad
        } else {
            PadMode::RightAlign
        };

        // `L` is only meaningful for float conversions; collapse to default otherwise.
        let length = (spec.length == LengthModifier::LongDouble && !spec.conversion.is_float())
            .then_some(LengthModifier::None)
            .unwrap_or(spec.length);

        Self {
            sign_mode,
            pad_mode,
            alternate: spec.flags.alternate,
            width,
            precision,
            length,
            conversion: spec.conversion,
        }
    }
}
