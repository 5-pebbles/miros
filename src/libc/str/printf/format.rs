use std::{
    ffi::{CStr, VaList},
    io::Write,
};

use super::specifier::{Conversion, LengthModifier, PadMode, ResolvedSpecifier};

pub struct Formatter<W: Write> {
    writer: W,
    bytes_written: usize,
    error: bool,
}

impl<W: Write> Formatter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            bytes_written: 0,
            error: false,
        }
    }

    pub fn finish(mut self) -> i32 {
        if !self.error && self.writer.flush().is_err() {
            return -1;
        }
        if self.error {
            return -1;
        }
        i32::try_from(self.bytes_written).unwrap_or(-1)
    }

    pub fn write_byte(&mut self, byte: u8) {
        if self.error {
            return;
        }
        if self.writer.write_all(&[byte]).is_err() {
            self.error = true;
        } else {
            self.bytes_written += 1;
        }
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        if self.error || bytes.is_empty() {
            return;
        }
        if self.writer.write_all(bytes).is_err() {
            self.error = true;
        } else {
            self.bytes_written += bytes.len();
        }
    }

    fn write_repeated(&mut self, byte: u8, mut count: usize) {
        if self.error || count == 0 {
            return;
        }
        let chunk = [byte; 64];
        while count > 0 {
            let batch = count.min(chunk.len());
            if self.writer.write_all(&chunk[..batch]).is_err() {
                self.error = true;
                return;
            }
            self.bytes_written += batch;
            count -= batch;
        }
    }

    pub unsafe fn format(&mut self, spec: &ResolvedSpecifier, args: &mut VaList<'_, '_>) {
        match spec.conversion {
            Conversion::SignedInt => self.format_signed_integer(spec, args),
            Conversion::UnsignedInt | Conversion::Octal | Conversion::Hex { .. } => {
                self.format_unsigned_integer(spec, args)
            }
            Conversion::String => self.format_string(spec, args),
            Conversion::Char => self.format_char(spec, args),
            Conversion::Pointer => self.format_pointer(spec, args),
            Conversion::CharCount => self.store_character_count(spec, args),
            Conversion::Float(_) => {
                // TODO: implement float formatting.
                // Consume the double from the VaList to keep subsequent arguments aligned.
                // C default argument promotion guarantees float → double in variadic calls.
                // NOTE: `L` (long double) args are wider than f64 on x86-64 (80-bit in a
                // 128-bit slot), so `%Lf` will misalign subsequent arguments until float
                // formatting is fully implemented.
                let _: f64 = args.arg();
            }
        }
    }

    unsafe fn format_signed_integer(
        &mut self,
        spec: &ResolvedSpecifier,
        args: &mut VaList<'_, '_>,
    ) {
        let value = spec.length.extract_signed(args);
        let radix = Radix::from_conversion(spec.conversion);
        self.write_integer(spec, radix, value.unsigned_abs(), value < 0);
    }

    unsafe fn format_unsigned_integer(
        &mut self,
        spec: &ResolvedSpecifier,
        args: &mut VaList<'_, '_>,
    ) {
        let value = spec.length.extract_unsigned(args);
        let radix = Radix::from_conversion(spec.conversion);
        self.write_integer(spec, radix, value, false);
    }

    /// Core integer renderer shared by all bases and signedness.
    fn write_integer(
        &mut self,
        spec: &ResolvedSpecifier,
        radix: Radix,
        value: u64,
        is_negative: bool,
    ) {
        let digit_buffer = DigitBuffer::from_value(value, radix);
        let sign = spec.sign_mode.sign_byte(is_negative);

        let prefix: &[u8] = if spec.alternate && value != 0 {
            radix.prefix()
        } else {
            b""
        };

        let digit_positions = radix.output_digit_count(
            digit_buffer.length,
            spec.precision,
            spec.alternate,
            value == 0,
        );

        let leading_zeros = digit_positions.saturating_sub(digit_buffer.length);

        let sign_len = sign.is_some() as usize;
        let content_width = sign_len + prefix.len() + digit_positions;
        let padding = spec.width.unwrap_or(0).saturating_sub(content_width);

        let (left_spaces, extra_zeros, right_spaces) = spec.pad_mode.resolve_padding(padding);

        self.write_repeated(b' ', left_spaces);
        if let Some(sign_char) = sign {
            self.write_byte(sign_char);
        }
        self.write_bytes(prefix);
        self.write_repeated(b'0', leading_zeros + extra_zeros);
        let digits = digit_buffer.digits();
        let emit_count = digit_positions.min(digits.len());
        for &digit in digits[..emit_count].iter().rev() {
            self.write_byte(digit);
        }
        self.write_repeated(b' ', right_spaces);
    }

    unsafe fn format_string(&mut self, spec: &ResolvedSpecifier, args: &mut VaList<'_, '_>) {
        let pointer: *const i8 = args.arg();

        let bytes: &[u8] = if pointer.is_null() {
            b"(null)"
        } else {
            CStr::from_ptr(pointer).to_bytes()
        };

        let effective_length = match spec.precision {
            Some(max) => bytes.len().min(max),
            None => bytes.len(),
        };

        self.write_padded(spec, effective_length, |formatter| {
            formatter.write_bytes(&bytes[..effective_length]);
        });
    }

    unsafe fn format_char(&mut self, spec: &ResolvedSpecifier, args: &mut VaList<'_, '_>) {
        let character = args.arg::<i32>() as u8;

        self.write_padded(spec, 1, |formatter| {
            formatter.write_byte(character);
        });
    }

    unsafe fn format_pointer(&mut self, spec: &ResolvedSpecifier, args: &mut VaList<'_, '_>) {
        let address = args.arg::<*const ()>().addr();

        if address == 0 {
            self.write_padded(spec, 5, |formatter| {
                formatter.write_bytes(b"(nil)");
            });
        } else {
            let pointer_spec = ResolvedSpecifier {
                alternate: true,
                ..*spec
            };
            self.write_integer(
                &pointer_spec,
                Radix::Hex { uppercase: false },
                address as u64,
                false,
            );
        }
    }

    unsafe fn store_character_count(
        &mut self,
        spec: &ResolvedSpecifier,
        args: &mut VaList<'_, '_>,
    ) {
        let count = self.bytes_written as i64;

        macro_rules! store_count {
            ($type:ty) => {{
                let destination: *mut $type = args.arg();
                if !destination.is_null() {
                    *destination = count as $type;
                }
            }};
        }

        match spec.length {
            LengthModifier::HalfHalf => store_count!(i8),
            LengthModifier::Half => store_count!(i16),
            LengthModifier::None | LengthModifier::LongDouble => store_count!(i32),
            LengthModifier::Long | LengthModifier::LongLong | LengthModifier::IntMax => {
                store_count!(i64)
            }
            LengthModifier::Size | LengthModifier::Ptrdiff => store_count!(isize),
        }
    }

    /// Write content within a space-padded field.
    fn write_padded(
        &mut self,
        spec: &ResolvedSpecifier,
        content_width: usize,
        emit_content: impl FnOnce(&mut Self),
    ) {
        let padding = spec.width.unwrap_or(0).saturating_sub(content_width);
        let left_align = spec.pad_mode == PadMode::LeftAlign;

        if !left_align {
            self.write_repeated(b' ', padding);
        }
        emit_content(self);
        if left_align {
            self.write_repeated(b' ', padding);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Radix {
    Decimal,
    Octal,
    Hex { uppercase: bool },
}

impl Radix {
    fn from_conversion(conversion: Conversion) -> Self {
        match conversion {
            Conversion::Octal => Self::Octal,
            Conversion::Hex { uppercase } => Self::Hex { uppercase },
            _ => Self::Decimal,
        }
    }

    fn base(self) -> u64 {
        match self {
            Self::Decimal => 10,
            Self::Octal => 8,
            Self::Hex { .. } => 16,
        }
    }

    /// The prefix characters for alternate form (`0x`, `0X`, or empty).
    /// Does NOT check whether alternate form is active — that's the caller's decision.
    fn prefix(self) -> &'static [u8] {
        match self {
            Self::Hex { uppercase: false } => b"0x",
            Self::Hex { uppercase: true } => b"0X",
            _ => b"",
        }
    }

    fn is_octal(self) -> bool {
        matches!(self, Self::Octal)
    }

    fn is_uppercase(self) -> bool {
        matches!(self, Self::Hex { uppercase: true })
    }

    /// Compute the number of digit positions to display, accounting for precision and the octal alternate-form rule.
    fn output_digit_count(
        self,
        actual_digits: usize,
        precision: Option<usize>,
        alternate: bool,
        value_is_zero: bool,
    ) -> usize {
        let effective_precision = precision.unwrap_or(1);

        if value_is_zero && effective_precision == 0 {
            // Precision 0 with value 0 suppresses all digits,
            // unless octal alternate forces a single "0".
            if alternate && self.is_octal() {
                1
            } else {
                0
            }
        } else {
            let count = effective_precision.max(actual_digits);
            // Octal alternate: "increase precision to force a leading zero".
            if alternate && self.is_octal() && !value_is_zero && count == actual_digits {
                count + 1
            } else {
                count
            }
        }
    }
}

/// Digit buffer for integer-to-text conversion. Stores digits least-significant first.
struct DigitBuffer {
    storage: [u8; 22], // max u64 in octal = 22 digits
    length: usize,
}

impl DigitBuffer {
    fn from_value(value: u64, radix: Radix) -> Self {
        let mut buffer = Self {
            storage: [0; 22],
            length: 0,
        };

        if value == 0 {
            buffer.storage[0] = b'0';
            buffer.length = 1;
        } else {
            let base = radix.base();
            let uppercase = radix.is_uppercase();
            let mut remaining = value;
            while remaining > 0 {
                let digit = (remaining % base) as u8;
                buffer.storage[buffer.length] = if digit < 10 {
                    b'0' + digit
                } else if uppercase {
                    b'A' + (digit - 10)
                } else {
                    b'a' + (digit - 10)
                };
                buffer.length += 1;
                remaining /= base;
            }
        }

        buffer
    }

    fn digits(&self) -> &[u8] {
        &self.storage[..self.length]
    }
}

#[cfg(test)]
#[allow(improper_ctypes_definitions)]
mod tests {
    use super::{super::specifier::SignMode, *};
    use crate::test_macros::eq_tests;

    macro_rules! cstr {
        ($s:literal) => {
            concat!($s, "\0").as_ptr() as *const i8
        };
    }

    macro_rules! format_integer_tests {
        (mod $mod_name:ident {
            $($name:ident, $value:expr, $negative:expr, $radix:expr,
                { $($field:ident: $fval:expr),* $(,)? }, $expected:expr);* $(;)?
        }) => {
            mod $mod_name {
                use super::*;
                $(
                    #[test]
                    fn $name() {
                        let mut output = Vec::new();
                        let spec = ResolvedSpecifier {
                            $($field: $fval,)*
                            ..Default::default()
                        };
                        {
                            let mut formatter = Formatter::new(&mut output);
                            formatter.write_integer(&spec, $radix, $value, $negative);
                        }
                        assert_eq!(output.as_slice(), $expected);
                    }
                )*
            }
        };
    }

    macro_rules! format_dispatch_tests {
        (mod $mod_name:ident {
            $($name:ident, { $($field:ident: $fval:expr),* $(,)? },
                ($($arg:expr),*), $expected:expr);* $(;)?
        }) => {
            mod $mod_name {
                use super::*;

                unsafe extern "C" fn format_to_vec(
                    spec: &ResolvedSpecifier, mut args: ...
                ) -> Vec<u8> {
                    let mut output = Vec::new();
                    {
                        let mut formatter = Formatter::new(&mut output);
                        formatter.format(spec, &mut args.as_va_list());
                    }
                    output
                }

                $(
                    #[test]
                    fn $name() {
                        let spec = ResolvedSpecifier {
                            $($field: $fval,)*
                            ..Default::default()
                        };
                        let output = unsafe { format_to_vec(&spec, $($arg),*) };
                        assert_eq!(output.as_slice(), $expected);
                    }
                )*
            }
        };
    }

    eq_tests!(mod radix {
        decimal_base,       Radix::Decimal.base(),                      10;
        octal_base,         Radix::Octal.base(),                        8;
        hex_base,           Radix::Hex { uppercase: false }.base(),     16;
        decimal_prefix,     Radix::Decimal.prefix(),                    b"" as &[u8];
        octal_prefix,       Radix::Octal.prefix(),                      b"" as &[u8];
        hex_lower_prefix,   Radix::Hex { uppercase: false }.prefix(),   b"0x" as &[u8];
        hex_upper_prefix,   Radix::Hex { uppercase: true }.prefix(),    b"0X" as &[u8];
        octal_is_octal,     Radix::Octal.is_octal(),                    true;
        decimal_not_octal,  Radix::Decimal.is_octal(),                  false;
        hex_upper_is_upper, Radix::Hex { uppercase: true }.is_uppercase(), true;
        hex_lower_not_upper, Radix::Hex { uppercase: false }.is_uppercase(), false;
        from_octal,         Radix::from_conversion(Conversion::Octal),  Radix::Octal;
        from_hex_lower,     Radix::from_conversion(Conversion::Hex { uppercase: false }), Radix::Hex { uppercase: false };
        from_signed,        Radix::from_conversion(Conversion::SignedInt), Radix::Decimal
    });

    eq_tests!(mod output_digit_count {
        default_uses_actual,        Radix::Decimal.output_digit_count(3, None, false, false),    3;
        default_minimum_one,        Radix::Decimal.output_digit_count(1, None, false, false),    1;
        precision_pads,             Radix::Decimal.output_digit_count(2, Some(5), false, false), 5;
        precision_does_not_shrink,  Radix::Decimal.output_digit_count(5, Some(2), false, false), 5;
        zero_value_zero_precision,  Radix::Decimal.output_digit_count(1, Some(0), false, true),  0;
        octal_alt_zero_value,       Radix::Octal.output_digit_count(1, Some(0), true, true),     1;
        octal_alt_forces_leading,   Radix::Octal.output_digit_count(3, None, true, false),       4;
        octal_alt_no_extra_if_padded, Radix::Octal.output_digit_count(3, Some(5), true, false),  5
    });

    eq_tests!(mod digit_buffer {
        zero_decimal,      DigitBuffer::from_value(0, Radix::Decimal).digits(),                  &[b'0'] as &[u8];
        forty_two_decimal, DigitBuffer::from_value(42, Radix::Decimal).digits(),                 &[b'2', b'4'] as &[u8];
        hex_lowercase,     DigitBuffer::from_value(255, Radix::Hex { uppercase: false }).digits(), &[b'f', b'f'] as &[u8];
        hex_uppercase,     DigitBuffer::from_value(255, Radix::Hex { uppercase: true }).digits(),  &[b'F', b'F'] as &[u8];
        octal_eight,       DigitBuffer::from_value(8, Radix::Octal).digits(),                    &[b'0', b'1'] as &[u8];
        single_digit,      DigitBuffer::from_value(7, Radix::Decimal).digits(),                  &[b'7']    });

    format_integer_tests!(mod integer_output {
        zero,                   0,   false, Radix::Decimal, {},                                    b"0" as &[u8];
        simple_positive,        42,  false, Radix::Decimal, {},                                    b"42" as &[u8];
        simple_negative,        42,  true,  Radix::Decimal, {},                                    b"-42" as &[u8];
        forced_sign_positive,   42,  false, Radix::Decimal, { sign_mode: SignMode::Force },        b"+42" as &[u8];
        space_sign_positive,    42,  false, Radix::Decimal, { sign_mode: SignMode::Space },        b" 42" as &[u8];
        right_padded,           42,  false, Radix::Decimal, { width: Some(8) },                    b"      42" as &[u8];
        left_padded,            42,  false, Radix::Decimal, { width: Some(8), pad_mode: PadMode::LeftAlign }, b"42      " as &[u8];
        zero_padded,            42,  false, Radix::Decimal, { width: Some(8), pad_mode: PadMode::ZeroPad },   b"00000042" as &[u8];
        precision_leading_zeros, 42, false, Radix::Decimal, { precision: Some(5) },                b"00042" as &[u8];
        hex_lower,              255, false, Radix::Hex { uppercase: false }, {},                    b"ff" as &[u8];
        hex_upper,              255, false, Radix::Hex { uppercase: true },  {},                    b"FF" as &[u8];
        hex_alternate,          255, false, Radix::Hex { uppercase: false }, { alternate: true },   b"0xff" as &[u8];
        octal_alternate,        8,   false, Radix::Octal, { alternate: true },                     b"010" as &[u8];
        zero_precision_zero,    0,   false, Radix::Decimal, { precision: Some(0) },                b"" as &[u8];
        octal_alt_zero,         0,   false, Radix::Octal, { precision: Some(0), alternate: true }, b"0"    });

    format_dispatch_tests!(mod format_signed {
        basic,            { conversion: Conversion::SignedInt },                (42i32),    b"42" as &[u8];
        negative,         { conversion: Conversion::SignedInt },                (-42i32),   b"-42" as &[u8];
        half_half_trunc,  { conversion: Conversion::SignedInt, length: LengthModifier::HalfHalf }, (0xFFi32), b"-1" as &[u8];
        long_value,       { conversion: Conversion::SignedInt, length: LengthModifier::Long },     (1234567890i64), b"1234567890"    });

    format_dispatch_tests!(mod format_unsigned {
        decimal,    { conversion: Conversion::UnsignedInt },                    (42u32),  b"42" as &[u8];
        hex_lower,  { conversion: Conversion::Hex { uppercase: false } },       (255u32), b"ff" as &[u8];
        octal,      { conversion: Conversion::Octal },                          (8u32),   b"10" as &[u8];
        hex_alt,    { conversion: Conversion::Hex { uppercase: false }, alternate: true }, (255u32), b"0xff"    });

    format_dispatch_tests!(mod format_string {
        basic,          { conversion: Conversion::String },                          (cstr!("hello")), b"hello" as &[u8];
        null_pointer,   { conversion: Conversion::String },                          (core::ptr::null::<i8>()),          b"(null)" as &[u8];
        with_precision, { conversion: Conversion::String, precision: Some(3) },      (cstr!("hello")), b"hel" as &[u8];
        right_padded,   { conversion: Conversion::String, width: Some(10) },         (cstr!("hi")),    b"        hi" as &[u8];
        left_padded,    { conversion: Conversion::String, width: Some(10), pad_mode: PadMode::LeftAlign }, (cstr!("hi")), b"hi        "    });

    format_dispatch_tests!(mod format_char {
        letter_a,   { conversion: Conversion::Char },                          (65i32), b"A" as &[u8];
        right_pad,  { conversion: Conversion::Char, width: Some(5) },          (65i32), b"    A" as &[u8];
        left_pad,   { conversion: Conversion::Char, width: Some(5), pad_mode: PadMode::LeftAlign }, (65i32), b"A    "    });

    format_dispatch_tests!(mod format_pointer {
        null_pointer, { conversion: Conversion::Pointer }, (core::ptr::null::<()>()), b"(nil)"    });
}
