use std::{
    ffi::{CStr, VaList},
    io::Write,
};

use super::{
    parse::{Conversion, LengthModifier, PadMode},
    ResolvedSpecifier,
};

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

#[derive(Clone, Copy)]
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
