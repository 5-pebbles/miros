use std::ffi::CStr;

/// Raw flags parsed from a printf format specifier.
/// No interpretation is applied — flag validity and interactions
/// are resolved later by `ResolvedSpecifier`.
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
    fn from_byte(byte: u8) -> Option<Self> {
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

/// An item yielded by [`PrintfParser`].
pub enum PrintfItem<'a> {
    Literal(&'a [u8]),
    Specifier(PrintfSpecifier),
}

/// Iterator that parses a C printf format string into [`PrintfItem`]s.
///
/// Inspired by relibc's `PrintfIter` — each call to `next()` yields either a
/// span of literal text or a fully-parsed specifier ready for argument extraction.
pub struct PrintfParser<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> PrintfParser<'a> {
    /// # Safety
    ///
    /// `format` must point to a valid, null-terminated C string that outlives `'a`.
    pub unsafe fn new(format: *const i8) -> Self {
        Self {
            bytes: CStr::from_ptr(format).to_bytes(),
            position: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let byte = self.bytes.get(self.position).copied()?;
        self.position += 1;
        Some(byte)
    }

    fn parse_specifier(&mut self) -> Option<PrintfSpecifier> {
        let flags = self.parse_raw_flags();
        let width = self.parse_dimension();
        let precision = self.parse_precision();
        let length = self.parse_length_modifier();
        let conversion = Conversion::from_byte(self.advance().unwrap_or(b'\0'))?;

        Some(PrintfSpecifier {
            flags,
            width,
            precision,
            length,
            conversion,
        })
    }

    fn parse_raw_flags(&mut self) -> RawFlags {
        let mut flags = RawFlags::default();
        loop {
            match self.peek() {
                Some(b'-') => flags.left_justify = true,
                Some(b'+') => flags.force_sign = true,
                Some(b' ') => flags.space_sign = true,
                Some(b'#') => flags.alternate = true,
                Some(b'0') => flags.zero_pad = true,
                _ => return flags,
            }
            self.advance();
        }
    }

    fn parse_dimension(&mut self) -> DimensionSpecifier {
        match self.peek() {
            Some(b'*') => {
                self.advance();
                DimensionSpecifier::FromNextArg
            }
            Some(b'1'..=b'9') => DimensionSpecifier::Fixed(self.parse_decimal()),
            _ => DimensionSpecifier::Unspecified,
        }
    }

    fn parse_precision(&mut self) -> DimensionSpecifier {
        if self.peek() != Some(b'.') {
            return DimensionSpecifier::Unspecified;
        }
        self.advance();
        match self.peek() {
            Some(b'*') => {
                self.advance();
                DimensionSpecifier::FromNextArg
            }
            Some(b'0'..=b'9') => DimensionSpecifier::Fixed(self.parse_decimal()),
            _ => DimensionSpecifier::Fixed(0),
        }
    }

    fn parse_length_modifier(&mut self) -> LengthModifier {
        match self.peek() {
            Some(b'h') => {
                self.advance();
                if self.peek() == Some(b'h') {
                    self.advance();
                    LengthModifier::HalfHalf
                } else {
                    LengthModifier::Half
                }
            }
            Some(b'l') => {
                self.advance();
                if self.peek() == Some(b'l') {
                    self.advance();
                    LengthModifier::LongLong
                } else {
                    LengthModifier::Long
                }
            }
            Some(b'z') => {
                self.advance();
                LengthModifier::Size
            }
            Some(b't') => {
                self.advance();
                LengthModifier::Ptrdiff
            }
            Some(b'j') => {
                self.advance();
                LengthModifier::IntMax
            }
            Some(b'L') => {
                self.advance();
                LengthModifier::LongDouble
            }
            _ => LengthModifier::None,
        }
    }

    fn parse_decimal(&mut self) -> usize {
        let mut value = 0_usize;
        while let Some(digit @ b'0'..=b'9') = self.peek() {
            value = value
                .saturating_mul(10)
                .saturating_add((digit - b'0') as usize);
            self.advance();
        }
        value
    }
}

impl<'a> Iterator for PrintfParser<'a> {
    type Item = PrintfItem<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let current = self.peek()?;

            if current == b'%' {
                self.advance();
                if self.peek() == Some(b'%') {
                    self.advance();
                    return Some(PrintfItem::Literal(b"%"));
                }
                if let Some(spec) = self.parse_specifier() {
                    return Some(PrintfItem::Specifier(spec));
                }
                // Unknown conversion specifier — already consumed, skip it
            } else {
                let start = self.position;
                while self.peek().is_some_and(|byte| byte != b'%') {
                    self.advance();
                }
                return Some(PrintfItem::Literal(&self.bytes[start..self.position]));
            }
        }
    }
}
