use std::ffi::VaList;

/// Raw flags parsed from a printf format specifier.
/// No interpretation is applied — flag validity and interactions are resolved later by `ResolvedSpecifier`.
#[derive(Default, Clone, Copy)]
pub struct RawFlags {
    pub left_justify: bool,
    pub force_sign: bool,
    pub space_sign: bool,
    pub alternate: bool,
    pub zero_pad: bool,
}

#[derive(Clone, Copy)]
pub enum SignMode {
    None,
    Space,
    Force,
}

impl SignMode {
    pub fn sign_byte(self, is_negative: bool) -> Option<u8> {
        if is_negative {
            Some(b'-')
        } else {
            match self {
                Self::Force => Some(b'+'),
                Self::Space => Some(b' '),
                Self::None => None,
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PadMode {
    RightAlign,
    LeftAlign,
    ZeroPad,
}

impl PadMode {
    /// Resolve where padding goes: `(left_spaces, extra_zeros, right_spaces)`.
    ///
    /// For zero-pad, the padding becomes additional zeros between sign/prefix and digits.
    /// For right-justify (default), padding is leading spaces.
    /// For left-justify, padding is trailing spaces.
    pub fn resolve_padding(self, padding: usize) -> (usize, usize, usize) {
        match self {
            Self::ZeroPad => (0, padding, 0),
            Self::LeftAlign => (0, 0, padding),
            Self::RightAlign => (padding, 0, 0),
        }
    }
}

/// Length modifier (`h`, `hh`, `l`, `ll`, `z`, `t`, `j`).
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum LengthModifier {
    #[default]
    None,
    HalfHalf,
    Half,
    Long,
    LongLong,
    LongDouble,
    Size,
    Ptrdiff,
    IntMax,
}

impl LengthModifier {
    pub unsafe fn extract_signed(self, args: &mut VaList<'_, '_>) -> i64 {
        match self {
            Self::HalfHalf => (args.arg::<i32>() as i8) as i64,
            Self::Half => (args.arg::<i32>() as i16) as i64,
            Self::None | Self::LongDouble => args.arg::<i32>() as i64,
            Self::Long | Self::LongLong | Self::IntMax => args.arg::<i64>(),
            Self::Size | Self::Ptrdiff => args.arg::<isize>() as i64,
        }
    }

    pub unsafe fn extract_unsigned(self, args: &mut VaList<'_, '_>) -> u64 {
        match self {
            Self::HalfHalf => (args.arg::<u32>() as u8) as u64,
            Self::Half => (args.arg::<u32>() as u16) as u64,
            Self::None | Self::LongDouble => args.arg::<u32>() as u64,
            Self::Long | Self::LongLong | Self::IntMax => args.arg::<u64>(),
            Self::Size | Self::Ptrdiff => args.arg::<usize>() as u64,
        }
    }
}

#[derive(Default, Clone, Copy)]
pub enum DimensionSpecifier {
    #[default]
    Unspecified,
    Fixed(usize),
    FromNextArg,
}

#[derive(Clone, Copy)]
pub enum Conversion {
    SignedInt,
    UnsignedInt,
    Octal,
    Hex { uppercase: bool },
    Float(FloatFormat),
    String,
    Char,
    Pointer,
    CharCount,
}

impl Conversion {
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            b'd' | b'i' => Some(Self::SignedInt),
            b'u' => Some(Self::UnsignedInt),
            b'o' => Some(Self::Octal),
            b'x' => Some(Self::Hex { uppercase: false }),
            b'X' => Some(Self::Hex { uppercase: true }),
            b'f' => Some(Self::Float(FloatFormat::Fixed { uppercase: false })),
            b'F' => Some(Self::Float(FloatFormat::Fixed { uppercase: true })),
            b'e' => Some(Self::Float(FloatFormat::Scientific { uppercase: false })),
            b'E' => Some(Self::Float(FloatFormat::Scientific { uppercase: true })),
            b'g' => Some(Self::Float(FloatFormat::General { uppercase: false })),
            b'G' => Some(Self::Float(FloatFormat::General { uppercase: true })),
            b'a' => Some(Self::Float(FloatFormat::Hex { uppercase: false })),
            b'A' => Some(Self::Float(FloatFormat::Hex { uppercase: true })),
            b's' => Some(Self::String),
            b'c' => Some(Self::Char),
            b'p' => Some(Self::Pointer),
            b'n' => Some(Self::CharCount),
            _ => None,
        }
    }

    pub fn is_signed(self) -> bool {
        matches!(self, Self::SignedInt | Self::Float(_))
    }

    pub fn is_float(self) -> bool {
        matches!(self, Self::Float(_))
    }

    pub fn is_integer(self) -> bool {
        matches!(
            self,
            Self::SignedInt | Self::UnsignedInt | Self::Octal | Self::Hex { .. }
        )
    }

    pub fn is_numeric(self) -> bool {
        self.is_integer() || matches!(self, Self::Float(_))
    }
}

#[derive(Clone, Copy)]
pub enum FloatFormat {
    Fixed { uppercase: bool },
    Scientific { uppercase: bool },
    General { uppercase: bool },
    Hex { uppercase: bool },
}

/// A parsed format specifier — everything between `%` and the conversion character.
pub struct PrintfSpecifier {
    pub flags: RawFlags,
    pub width: DimensionSpecifier,
    pub precision: DimensionSpecifier,
    pub length: LengthModifier,
    pub conversion: Conversion,
}

/// Fully resolved specifier with all flag interactions applied.
/// Produced from a raw `PrintfSpecifier` by resolving `*` dimensions
/// from the argument list and applying C11 §7.21.6.1p6 rules.
#[derive(Clone, Copy)]
pub struct ResolvedSpecifier {
    pub sign_mode: SignMode,
    pub pad_mode: PadMode,
    pub alternate: bool,
    pub width: Option<usize>,
    pub precision: Option<usize>,
    pub length: LengthModifier,
    pub conversion: Conversion,
}

impl ResolvedSpecifier {
    /// Resolve `*` width/precision from the argument list and apply all flag-interaction rules (C11 §7.21.6.1p6).
    pub unsafe fn from_parsed(spec: PrintfSpecifier, args: &mut VaList<'_, '_>) -> Self {
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
