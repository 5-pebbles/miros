use std::ffi::VaList;

/// Raw flags parsed from a printf format specifier.
/// No interpretation is applied — flag validity and interactions are resolved later by `ResolvedSpecifier`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RawFlags {
    pub left_justify: bool,
    pub force_sign: bool,
    pub space_sign: bool,
    pub alternate: bool,
    pub zero_pad: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SignMode {
    #[default]
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PadMode {
    #[default]
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
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
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
    pub unsafe fn extract_signed(self, args: &mut VaList<'_>) -> i64 {
        match self {
            Self::HalfHalf => (args.arg::<i32>() as i8) as i64,
            Self::Half => (args.arg::<i32>() as i16) as i64,
            Self::None | Self::LongDouble => args.arg::<i32>() as i64,
            Self::Long | Self::LongLong | Self::IntMax => args.arg::<i64>(),
            Self::Size | Self::Ptrdiff => args.arg::<isize>() as i64,
        }
    }

    pub unsafe fn extract_unsigned(self, args: &mut VaList<'_>) -> u64 {
        match self {
            Self::HalfHalf => (args.arg::<u32>() as u8) as u64,
            Self::Half => (args.arg::<u32>() as u16) as u64,
            Self::None | Self::LongDouble => args.arg::<u32>() as u64,
            Self::Long | Self::LongLong | Self::IntMax => args.arg::<u64>(),
            Self::Size | Self::Ptrdiff => args.arg::<usize>() as u64,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DimensionSpecifier {
    #[default]
    Unspecified,
    Fixed(usize),
    FromNextArg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatFormat {
    Fixed { uppercase: bool },
    Scientific { uppercase: bool },
    General { uppercase: bool },
    Hex { uppercase: bool },
}

/// A parsed format specifier — everything between `%` and the conversion character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub unsafe fn from_parsed(spec: PrintfSpecifier, args: &mut VaList<'_>) -> Self {
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

#[cfg(test)]
#[allow(improper_ctypes_definitions)]
mod tests {
    use super::*;
    use crate::test_macros::eq_tests;

    /// Test-only defaults: `Conversion::SignedInt` is an arbitrary choice for `..Default::default()` ergonomics in test macros.
    /// Not available in production to avoid masking missing fields.
    impl Default for Conversion {
        fn default() -> Self {
            Self::SignedInt
        }
    }

    impl Default for PrintfSpecifier {
        fn default() -> Self {
            Self {
                flags: RawFlags::default(),
                width: DimensionSpecifier::default(),
                precision: DimensionSpecifier::default(),
                length: LengthModifier::default(),
                conversion: Conversion::default(),
            }
        }
    }

    impl Default for ResolvedSpecifier {
        fn default() -> Self {
            Self {
                sign_mode: SignMode::default(),
                pad_mode: PadMode::default(),
                alternate: false,
                width: None,
                precision: None,
                length: LengthModifier::default(),
                conversion: Conversion::default(),
            }
        }
    }

    macro_rules! predicate_tests {
        (mod $mod_name:ident {
            $($name:ident, $conversion:expr,
                ($signed:expr, $float:expr, $integer:expr, $numeric:expr));* $(;)?
        }) => {
            mod $mod_name {
                use super::*;
                $(
                    #[test]
                    fn $name() {
                        let conversion = $conversion;
                        assert_eq!(
                            (
                                conversion.is_signed(),
                                conversion.is_float(),
                                conversion.is_integer(),
                                conversion.is_numeric(),
                            ),
                            ($signed, $float, $integer, $numeric),
                        );
                    }
                )*
            }
        };
    }

    macro_rules! resolve_tests {
        (mod $mod_name:ident {
            $($name:ident,
                { $($spec_field:ident: $spec_val:expr),* $(,)? }
                ($($arg:expr),*)
                => { $($($path:ident).+ == $expected:expr),+ $(,)? });* $(;)?
        }) => {
            mod $mod_name {
                use super::*;

                unsafe extern "C" fn resolve(
                    spec: PrintfSpecifier, mut args: ...
                ) -> ResolvedSpecifier {
                    ResolvedSpecifier::from_parsed(spec, &mut args)
                }

                $(
                    #[test]
                    fn $name() {
                        let spec = PrintfSpecifier {
                            $($spec_field: $spec_val,)*
                            ..Default::default()
                        };
                        let resolved = unsafe { resolve(spec, $($arg),*) };
                        $(assert_eq!(resolved.$($path).+, $expected);)+
                    }
                )*
            }
        };
    }

    unsafe extern "C" fn extract_signed_va(length: LengthModifier, mut args: ...) -> i64 {
        length.extract_signed(&mut args)
    }

    unsafe extern "C" fn extract_unsigned_va(length: LengthModifier, mut args: ...) -> u64 {
        length.extract_unsigned(&mut args)
    }

    eq_tests!(mod sign_byte {
        none_positive,  SignMode::None.sign_byte(false),  None;
        none_negative,  SignMode::None.sign_byte(true),   Some(b'-');
        space_positive, SignMode::Space.sign_byte(false), Some(b' ');
        space_negative, SignMode::Space.sign_byte(true),  Some(b'-');
        force_positive, SignMode::Force.sign_byte(false), Some(b'+');
        force_negative, SignMode::Force.sign_byte(true),  Some(b'-')
    });

    eq_tests!(mod resolve_padding {
        right_align, PadMode::RightAlign.resolve_padding(5), (5, 0, 0);
        left_align,  PadMode::LeftAlign.resolve_padding(5),  (0, 0, 5);
        zero_pad,    PadMode::ZeroPad.resolve_padding(5),    (0, 5, 0)
    });

    //                                                    signed  float   integer numeric
    predicate_tests!(mod conversion_predicates {
        signed_int,   Conversion::SignedInt,              (true,  false, true,  true);
        unsigned_int, Conversion::UnsignedInt,            (false, false, true,  true);
        octal,        Conversion::Octal,                  (false, false, true,  true);
        hex,          Conversion::Hex { uppercase: false }, (false, false, true, true);
        float_fixed,  Conversion::Float(FloatFormat::Fixed { uppercase: false }), (true, true, false, true);
        string,       Conversion::String,                 (false, false, false, false);
        char_conv,    Conversion::Char,                   (false, false, false, false);
        pointer,      Conversion::Pointer,                (false, false, false, false);
        char_count,   Conversion::CharCount,              (false, false, false, false)
    });

    eq_tests!(mod from_byte {
        d_is_signed,          Conversion::from_byte(b'd'), Some(Conversion::SignedInt);
        i_is_signed,          Conversion::from_byte(b'i'), Some(Conversion::SignedInt);
        u_is_unsigned,        Conversion::from_byte(b'u'), Some(Conversion::UnsignedInt);
        o_is_octal,           Conversion::from_byte(b'o'), Some(Conversion::Octal);
        x_is_hex_lower,       Conversion::from_byte(b'x'), Some(Conversion::Hex { uppercase: false });
        x_is_hex_upper,       Conversion::from_byte(b'X'), Some(Conversion::Hex { uppercase: true });
        f_is_fixed_lower,     Conversion::from_byte(b'f'), Some(Conversion::Float(FloatFormat::Fixed { uppercase: false }));
        f_is_fixed_upper,     Conversion::from_byte(b'F'), Some(Conversion::Float(FloatFormat::Fixed { uppercase: true }));
        e_is_sci_lower,       Conversion::from_byte(b'e'), Some(Conversion::Float(FloatFormat::Scientific { uppercase: false }));
        e_is_sci_upper,       Conversion::from_byte(b'E'), Some(Conversion::Float(FloatFormat::Scientific { uppercase: true }));
        g_is_general_lower,   Conversion::from_byte(b'g'), Some(Conversion::Float(FloatFormat::General { uppercase: false }));
        g_is_general_upper,   Conversion::from_byte(b'G'), Some(Conversion::Float(FloatFormat::General { uppercase: true }));
        a_is_hex_float_lower, Conversion::from_byte(b'a'), Some(Conversion::Float(FloatFormat::Hex { uppercase: false }));
        a_is_hex_float_upper, Conversion::from_byte(b'A'), Some(Conversion::Float(FloatFormat::Hex { uppercase: true }));
        s_is_string,          Conversion::from_byte(b's'), Some(Conversion::String);
        c_is_char,            Conversion::from_byte(b'c'), Some(Conversion::Char);
        p_is_pointer,         Conversion::from_byte(b'p'), Some(Conversion::Pointer);
        n_is_char_count,      Conversion::from_byte(b'n'), Some(Conversion::CharCount);
        invalid_returns_none, Conversion::from_byte(b'Q'), None
    });

    eq_tests!(mod extract_signed {
        none_positive,           unsafe { extract_signed_va(LengthModifier::None, 42i32) },        42i64;
        half_half_sign_extends,  unsafe { extract_signed_va(LengthModifier::HalfHalf, 0xFFi32) },  -1i64;
        half_sign_extends,       unsafe { extract_signed_va(LengthModifier::Half, 0xFFFFi32) },    -1i64;
        long_passthrough,        unsafe { extract_signed_va(LengthModifier::Long, 42i64) },        42i64;
        long_long_passthrough,   unsafe { extract_signed_va(LengthModifier::LongLong, 42i64) },    42i64;
        long_double_as_int,      unsafe { extract_signed_va(LengthModifier::LongDouble, 42i32) },  42i64;
        intmax_passthrough,      unsafe { extract_signed_va(LengthModifier::IntMax, 42i64) },      42i64;
        size_extends,            unsafe { extract_signed_va(LengthModifier::Size, -1isize) },      -1i64;
        ptrdiff_extends,         unsafe { extract_signed_va(LengthModifier::Ptrdiff, -1isize) },   -1i64
    });

    eq_tests!(mod extract_unsigned {
        none_positive,         unsafe { extract_unsigned_va(LengthModifier::None, 42u32) },        42u64;
        half_half_truncates,   unsafe { extract_unsigned_va(LengthModifier::HalfHalf, 0x1FFu32) }, 0xFFu64;
        half_truncates,        unsafe { extract_unsigned_va(LengthModifier::Half, 0x1FFFFu32) },   0xFFFFu64;
        long_passthrough,      unsafe { extract_unsigned_va(LengthModifier::Long, 42u64) },        42u64;
        long_long_passthrough, unsafe { extract_unsigned_va(LengthModifier::LongLong, 42u64) },    42u64;
        long_double_as_int,    unsafe { extract_unsigned_va(LengthModifier::LongDouble, 42u32) },  42u64;
        intmax_passthrough,    unsafe { extract_unsigned_va(LengthModifier::IntMax, 42u64) },      42u64;
        size_extends,          unsafe { extract_unsigned_va(LengthModifier::Size, 42usize) },      42u64;
        ptrdiff_extends,       unsafe { extract_unsigned_va(LengthModifier::Ptrdiff, 42usize) },   42u64
    });

    resolve_tests!(mod from_parsed {
        force_sign_overrides_space,
            { flags: RawFlags { force_sign: true, space_sign: true, ..Default::default() } }
            ()
            => { sign_mode == SignMode::Force };

        space_sign_when_no_force,
            { flags: RawFlags { space_sign: true, ..Default::default() } }
            ()
            => { sign_mode == SignMode::Space };

        sign_mode_none_for_unsigned,
            { flags: RawFlags { force_sign: true, ..Default::default() }, conversion: Conversion::UnsignedInt }
            ()
            => { sign_mode == SignMode::None };

        left_justify_overrides_zero_pad,
            { flags: RawFlags { left_justify: true, zero_pad: true, ..Default::default() } }
            ()
            => { pad_mode == PadMode::LeftAlign };

        zero_pad_on_numeric,
            { flags: RawFlags { zero_pad: true, ..Default::default() } }
            ()
            => { pad_mode == PadMode::ZeroPad };

        zero_pad_ignored_on_non_numeric,
            { flags: RawFlags { zero_pad: true, ..Default::default() }, conversion: Conversion::String }
            ()
            => { pad_mode == PadMode::RightAlign };

        precision_suppresses_zero_pad_for_integers,
            { flags: RawFlags { zero_pad: true, ..Default::default() }, precision: DimensionSpecifier::Fixed(5) }
            ()
            => { pad_mode == PadMode::RightAlign };

        precision_does_not_suppress_zero_pad_for_floats,
            {
                flags: RawFlags { zero_pad: true, ..Default::default() },
                precision: DimensionSpecifier::Fixed(5),
                conversion: Conversion::Float(FloatFormat::Fixed { uppercase: false }),
            }
            ()
            => { pad_mode == PadMode::ZeroPad };

        negative_star_width_flips_align,
            { width: DimensionSpecifier::FromNextArg }
            (-10i32)
            => { width == Some(10), pad_mode == PadMode::LeftAlign };

        negative_star_precision_means_unspecified,
            { precision: DimensionSpecifier::FromNextArg }
            (-1i32)
            => { precision == None };

        long_double_collapses_for_non_float,
            { length: LengthModifier::LongDouble }
            ()
            => { length == LengthModifier::None };

        long_double_preserved_for_float,
            {
                length: LengthModifier::LongDouble,
                conversion: Conversion::Float(FloatFormat::Fixed { uppercase: false }),
            }
            ()
            => { length == LengthModifier::LongDouble };

        alternate_flag_passthrough,
            { flags: RawFlags { alternate: true, ..Default::default() } }
            ()
            => { alternate == true };

        positive_star_width,
            { width: DimensionSpecifier::FromNextArg }
            (10i32)
            => { width == Some(10), pad_mode == PadMode::RightAlign };

        positive_star_precision,
            { precision: DimensionSpecifier::FromNextArg }
            (5i32)
            => { precision == Some(5) };

        fixed_width,
            { width: DimensionSpecifier::Fixed(10) }
            ()
            => { width == Some(10) };

        fixed_precision,
            { precision: DimensionSpecifier::Fixed(5) }
            ()
            => { precision == Some(5) }
    });
}
