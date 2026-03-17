use std::ffi::CStr;

use super::specifier::{Conversion, DimensionSpecifier, LengthModifier, PrintfSpecifier, RawFlags};

/// An item yielded by [`PrintfParser`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::{super::specifier::FloatFormat, *};

    /// Generates a module of tests that each parse a single specifier and assert one or more fields on the result.
    macro_rules! specifier_tests {
        (mod $mod_name:ident {
            $($name:ident, $input:expr, { $($($path:ident).+ == $expected:expr),+ $(,)? });* $(;)?
        }) => {
            mod $mod_name {
                use super::*;
                $(
                    #[test]
                    fn $name() {
                        let format =
                            CString::new($input).expect("format should contain no internal nulls");
                        let mut parser = unsafe { PrintfParser::new(format.as_ptr()) };
                        let Some(PrintfItem::Specifier(specifier)) = parser.next() else {
                            panic!("expected a single Specifier item");
                        };
                        assert_eq!(parser.next(), None);
                        $(assert_eq!(specifier.$($path).+, $expected);)+
                    }
                )*
            }
        };
    }

    /// Generates a module of tests for `parse_decimal`.
    macro_rules! parse_decimal_tests {
        (mod $mod_name:ident {
            $($name:ident, $input:expr, $expected:expr $(, position == $pos:expr)?);* $(;)?
        }) => {
            mod $mod_name {
                use super::*;
                $(
                    #[test]
                    fn $name() {
                        let format =
                            CString::new($input).expect("format should contain no internal nulls");
                        let mut parser = unsafe { PrintfParser::new(format.as_ptr()) };
                        assert_eq!(parser.parse_decimal(), $expected);
                        $(assert_eq!(parser.position, $pos);)?
                    }
                )*
            }
        };
    }

    /// Generates a module of tests that collect all parsed items and assert on the resulting vec's length and optionally an element.
    macro_rules! parse_items_tests {
        (mod $mod_name:ident {
            $($name:ident, $input:expr, $expected_len:expr $(, [$idx:expr] == $expected:expr)?);* $(;)?
        }) => {
            mod $mod_name {
                use super::*;
                $(
                    #[test]
                    fn $name() {
                        let format =
                            CString::new($input).expect("format should contain no internal nulls");
                        let items: Vec<_> =
                            unsafe { PrintfParser::new(format.as_ptr()) }.collect();
                        assert_eq!(items.len(), $expected_len);
                        $(assert_eq!(items[$idx], $expected);)?
                    }
                )*
            }
        };
    }

    specifier_tests!(mod flags {
        no_flags,        "%d",      {
            flags.left_justify == false,
            flags.force_sign == false,
            flags.space_sign == false,
            flags.alternate == false,
            flags.zero_pad == false };
        left_justify,    "%-d",     {
            flags.left_justify == true,
            flags.force_sign == false,
            flags.space_sign == false,
            flags.alternate == false,
            flags.zero_pad == false };
        force_sign,      "%+d",     {
            flags.left_justify == false,
            flags.force_sign == true,
            flags.space_sign == false,
            flags.alternate == false,
            flags.zero_pad == false };
        space_sign,      "% d",     {
            flags.left_justify == false,
            flags.force_sign == false,
            flags.space_sign == true,
            flags.alternate == false,
            flags.zero_pad == false };
        alternate,       "%#d",     {
            flags.left_justify == false,
            flags.force_sign == false,
            flags.space_sign == false,
            flags.alternate == true,
            flags.zero_pad == false };
        zero_pad,        "%0d",     {
            flags.left_justify == false,
            flags.force_sign == false,
            flags.space_sign == false,
            flags.alternate == false,
            flags.zero_pad == true  };
        all_flags,       "%-+ #0d", {
            flags.left_justify == true,
            flags.force_sign == true,
            flags.space_sign == true,
            flags.alternate == true,
            flags.zero_pad == true  };
        duplicate_flags, "%--++d",  {
            flags.left_justify == true,
            flags.force_sign == true,
            flags.space_sign == false,
            flags.alternate == false,
            flags.zero_pad == false }
    });

    specifier_tests!(mod precision {
        unspecified_without_dot, "%d",   { precision == DimensionSpecifier::Unspecified };
        dot_alone_means_zero,    "%.d",  { precision == DimensionSpecifier::Fixed(0) };
        dot_with_digits,         "%.6d", { precision == DimensionSpecifier::Fixed(6) };
        dot_with_zero,           "%.0d", { precision == DimensionSpecifier::Fixed(0) };
        dot_with_star,           "%.*d", { precision == DimensionSpecifier::FromNextArg }
    });

    specifier_tests!(mod length_modifiers {
        none,        "%d",   { length == LengthModifier::None };
        half,        "%hd",  { length == LengthModifier::Half };
        half_half,   "%hhd", { length == LengthModifier::HalfHalf };
        long,        "%ld",  { length == LengthModifier::Long };
        long_long,   "%lld", { length == LengthModifier::LongLong };
        size,        "%zd",  { length == LengthModifier::Size };
        ptrdiff,     "%td",  { length == LengthModifier::Ptrdiff };
        intmax,      "%jd",  { length == LengthModifier::IntMax };
        long_double, "%Lf",  { length == LengthModifier::LongDouble }
    });

    specifier_tests!(mod conversions {
        signed_d,               "%d", { conversion == Conversion::SignedInt };
        signed_i,               "%i", { conversion == Conversion::SignedInt };
        unsigned,               "%u", { conversion == Conversion::UnsignedInt };
        octal,                  "%o", { conversion == Conversion::Octal };
        hex_lower,              "%x", { conversion == Conversion::Hex { uppercase: false } };
        hex_upper,              "%X", { conversion == Conversion::Hex { uppercase: true } };
        float_fixed_lower,      "%f", { conversion == Conversion::Float(FloatFormat::Fixed { uppercase: false }) };
        float_fixed_upper,      "%F", { conversion == Conversion::Float(FloatFormat::Fixed { uppercase: true }) };
        float_scientific_lower, "%e", { conversion == Conversion::Float(FloatFormat::Scientific { uppercase: false }) };
        float_scientific_upper, "%E", { conversion == Conversion::Float(FloatFormat::Scientific { uppercase: true }) };
        float_general_lower,    "%g", { conversion == Conversion::Float(FloatFormat::General { uppercase: false }) };
        float_general_upper,    "%G", { conversion == Conversion::Float(FloatFormat::General { uppercase: true }) };
        float_hex_lower,        "%a", { conversion == Conversion::Float(FloatFormat::Hex { uppercase: false }) };
        float_hex_upper,        "%A", { conversion == Conversion::Float(FloatFormat::Hex { uppercase: true }) };
        string,                 "%s", { conversion == Conversion::String };
        char,                   "%c", { conversion == Conversion::Char };
        pointer,                "%p", { conversion == Conversion::Pointer };
        char_count,             "%n", { conversion == Conversion::CharCount }
    });

    specifier_tests!(mod combined_specifiers {
        full_specifier_with_all_parts, "%-+10.5ld", {
            flags.left_justify == true,
            flags.force_sign == true,
            width == DimensionSpecifier::Fixed(10),
            precision == DimensionSpecifier::Fixed(5),
            length == LengthModifier::Long,
            conversion == Conversion::SignedInt,
        };
        star_width_and_star_precision, "%*.*d", {
            width == DimensionSpecifier::FromNextArg,
            precision == DimensionSpecifier::FromNextArg,
            conversion == Conversion::SignedInt,
        };
        alternate_hex, "%#x", {
            flags.alternate == true,
            conversion == Conversion::Hex { uppercase: false },
        }
    });

    parse_decimal_tests!(mod parse_decimal {
        single_digit,             "7",    7;
        multi_digit,              "123",  123;
        stops_at_non_digit,       "42abc", 42, position == 2;
        empty_input_returns_zero, "",     0;
        large_value_saturates,    "99999999999999999999999999999999", usize::MAX
    });

    // Edge cases derived from C11 §7.21.6.1.
    // Each test exercises a parser code path that no other test covers.
    parse_items_tests!(mod spec_edge_cases {
        percent_as_conversion_after_width_drops_specifier,  "%5%",    0;
        dot_after_precision_is_unrecognized_conversion,     "%.5.3d", 1, [0] == PrintfItem::Literal(b"3d");
        h_after_l_modifier_is_unrecognized_conversion,      "%lhd",   1, [0] == PrintfItem::Literal(b"d");
        truncated_after_width,                              "%10",    0
    });
}
