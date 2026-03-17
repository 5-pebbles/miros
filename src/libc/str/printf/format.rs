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
                        assert_eq!(output, $expected.as_bytes());
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
                        assert_eq!(output.as_slice(), $expected.as_bytes());
                    }
                )*
            }
        };
    }

    format_integer_tests!(mod integer_output {
        // Basic value rendering
        zero_value_default_precision_emits_single_zero,
            0,   false, Radix::Decimal, {}, "0";
        unsigned_value_no_flags,
            42,  false, Radix::Decimal, {}, "42";
        negative_value_emits_minus_sign,
            42,  true,  Radix::Decimal, {}, "-42";

        // Sign modes
        force_sign_emits_plus_for_positive,
            42,  false, Radix::Decimal, { sign_mode: SignMode::Force }, "+42";
        space_sign_emits_space_for_positive,
            42,  false, Radix::Decimal, { sign_mode: SignMode::Space }, " 42";

        // Padding and alignment
        default_alignment_right_pads_with_spaces,
            42,  false, Radix::Decimal, { width: Some(8) }, "      42";
        left_align_pads_trailing_spaces,
            42,  false, Radix::Decimal, { width: Some(8), pad_mode: PadMode::LeftAlign }, "42      ";
        zero_pad_fills_between_sign_and_digits,
            42,  false, Radix::Decimal, { width: Some(8), pad_mode: PadMode::ZeroPad }, "00000042";
        width_narrower_than_content_produces_no_padding,
            12345, false, Radix::Decimal, { width: Some(2) }, "12345";

        // Precision
        precision_pads_with_leading_zeros,
            42,  false, Radix::Decimal, { precision: Some(5) }, "00042";
        precision_zero_suppresses_zero_value,
            0,   false, Radix::Decimal, { precision: Some(0) }, "";
        precision_exceeding_width_expands_output,
            42,  false, Radix::Decimal, { precision: Some(10), width: Some(5) }, "0000000042";

        // Hex output
        hex_lowercase_digits,
            255, false, Radix::Hex { uppercase: false }, {}, "ff";
        hex_uppercase_digits,
            255, false, Radix::Hex { uppercase: true },  {}, "FF";
        alternate_hex_prepends_0x_prefix,
            255, false, Radix::Hex { uppercase: false }, { alternate: true }, "0xff";
        alternate_hex_zero_value_no_prefix,
            0,   false, Radix::Hex { uppercase: false }, { alternate: true }, "0";

        // Octal output
        alternate_octal_forces_leading_zero,
            8,   false, Radix::Octal, { alternate: true }, "010";
        alternate_octal_emits_zero_despite_zero_precision,
            0,   false, Radix::Octal, { precision: Some(0), alternate: true }, "0";
        alternate_octal_precision_already_provides_leading_zero,
            8,   false, Radix::Octal, { precision: Some(5), alternate: true }, "00010";

        // Multi-feature interactions
        zero_pad_with_sign_places_sign_before_zeros,
            42,  true,  Radix::Decimal, { width: Some(8), pad_mode: PadMode::ZeroPad }, "-0000042";
        precision_and_width_and_sign_interact,
            42,  false, Radix::Decimal, { sign_mode: SignMode::Force, width: Some(10), precision: Some(5) }, "    +00042";
        alternate_hex_with_zero_pad_and_width,
            255, false, Radix::Hex { uppercase: false }, { alternate: true, width: Some(10), pad_mode: PadMode::ZeroPad }, "0x000000ff";

        // Boundary values
        u64_max_decimal,
            u64::MAX, false, Radix::Decimal, {}, "18446744073709551615";
        u64_max_hex,
            u64::MAX, false, Radix::Hex { uppercase: false }, {}, "ffffffffffffffff"
    });

    format_dispatch_tests!(mod format_signed {
        half_half_truncates_to_signed_byte,
            { conversion: Conversion::SignedInt, length: LengthModifier::HalfHalf },
            (0xFFi32), "-1";
        long_modifier_extracts_64bit_signed,
            { conversion: Conversion::SignedInt, length: LengthModifier::Long },
            (1234567890i64), "1234567890"
    });

    format_dispatch_tests!(mod format_unsigned {
        alternate_hex_dispatches_unsigned,
            { conversion: Conversion::Hex { uppercase: false }, alternate: true },
            (255u32), "0xff"
    });

    format_dispatch_tests!(mod format_string {
        simple_string,
            { conversion: Conversion::String },
            (c"hello".as_ptr()), "hello";
        null_string_pointer_emits_null_sentinel,
            { conversion: Conversion::String },
            (core::ptr::null::<i8>()), "(null)";
        precision_truncates_string,
            { conversion: Conversion::String, precision: Some(3) },
            (c"hello".as_ptr()), "hel";
        default_alignment_right_pads_string,
            { conversion: Conversion::String, width: Some(10) },
            (c"hi".as_ptr()), "        hi";
        left_align_pads_string,
            { conversion: Conversion::String, width: Some(10), pad_mode: PadMode::LeftAlign },
            (c"hi".as_ptr()), "hi        "
    });

    format_dispatch_tests!(mod format_char {
        char_from_integer_code,
            { conversion: Conversion::Char },
            (65i32), "A";
        default_alignment_right_pads_char,
            { conversion: Conversion::Char, width: Some(5) },
            (65i32), "    A";
        left_align_pads_char,
            { conversion: Conversion::Char, width: Some(5), pad_mode: PadMode::LeftAlign },
            (65i32), "A    "
    });

    format_dispatch_tests!(mod format_pointer {
        null_address_emits_nil_sentinel,
            { conversion: Conversion::Pointer },
            (core::ptr::null::<()>()), "(nil)";
        non_null_address_emits_hex_with_prefix,
            { conversion: Conversion::Pointer },
            (0xDEADusize as *const ()), "0xdead"
    });

    mod format_char_count {
        use super::*;

        unsafe extern "C" fn format_n_to_vec(
            spec: &ResolvedSpecifier,
            bytes_already_written: usize,
            mut args: ...
        ) -> Vec<u8> {
            let mut output = Vec::new();
            {
                let mut formatter = Formatter::new(&mut output);
                formatter.bytes_written = bytes_already_written;
                formatter.format(spec, &mut args.as_va_list());
            }
            output
        }

        #[test]
        fn stores_current_byte_count_at_pointer() {
            let mut count: i32 = -1;
            let spec = ResolvedSpecifier {
                conversion: Conversion::CharCount,
                ..Default::default()
            };
            let output = unsafe { format_n_to_vec(&spec, 5, &mut count as *mut i32) };
            assert!(output.is_empty());
            assert_eq!(count, 5);
        }

        #[test]
        fn zero_bytes_written_stores_zero() {
            let mut count: i32 = -1;
            let spec = ResolvedSpecifier {
                conversion: Conversion::CharCount,
                ..Default::default()
            };
            let output = unsafe { format_n_to_vec(&spec, 0, &mut count as *mut i32) };
            assert!(output.is_empty());
            assert_eq!(count, 0);
        }
    }

    mod finish_return_value {
        use super::*;

        #[test]
        fn returns_total_bytes_written() {
            let mut output = Vec::new();
            let formatter = Formatter::new(&mut output);
            assert_eq!(formatter.finish(), 0);

            let mut output = Vec::new();
            {
                let mut formatter = Formatter::new(&mut output);
                formatter.write_bytes(b"hello");
                assert_eq!(formatter.finish(), 5);
            }
            assert_eq!(output, b"hello");
        }

        #[test]
        fn accumulates_across_multiple_writes() {
            let mut output = Vec::new();
            {
                let mut formatter = Formatter::new(&mut output);
                formatter.write_bytes(b"abc");
                formatter.write_byte(b'd');
                formatter.write_bytes(b"ef");
                assert_eq!(formatter.finish(), 6);
            }
            assert_eq!(output, b"abcdef");
        }
    }
}
