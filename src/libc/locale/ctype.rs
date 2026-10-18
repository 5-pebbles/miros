use core::ffi::{c_int, c_uint};

use bitbybit::bitfield;

/// glibc's `_ISbit` layout: the high byte holds most tests, the low nibble the rest.
#[bitfield(u16)]
struct CTypeFlags {
    #[bit(8, rw)]
    upper: bool,
    #[bit(9, rw)]
    lower: bool,
    #[bit(10, rw)]
    alpha: bool,
    #[bit(11, rw)]
    digit: bool,
    #[bit(12, rw)]
    xdigit: bool,
    #[bit(13, rw)]
    space: bool,
    #[bit(14, rw)]
    print: bool,
    #[bit(15, rw)]
    graph: bool,
    #[bit(0, rw)]
    blank: bool,
    #[bit(1, rw)]
    cntrl: bool,
    #[bit(2, rw)]
    punct: bool,
    #[bit(3, rw)]
    alnum: bool,
}

// glibc indexes the table with signed char: entries 0..128 cover -128..-1, then 0..255.
const CTYPE_TABLE_LENGTH: usize = 384;
const CTYPE_TABLE_INDEX_OF_ZERO: usize = 128;

const fn flags_for(byte: u8) -> u16 {
    let upper = byte.is_ascii_uppercase();
    let lower = byte.is_ascii_lowercase();
    let alpha = upper || lower;
    let digit = byte.is_ascii_digit();
    let alnum = alpha || digit;
    let graph = byte.is_ascii_graphic();

    // Rust's ASCII whitespace omits vertical tab; glibc's `isspace` includes it.
    let space = byte.is_ascii_whitespace() || byte == 0x0b;

    CTypeFlags::new_with_raw_value(0)
        .with_upper(upper)
        .with_lower(lower)
        .with_alpha(alpha)
        .with_digit(digit)
        .with_xdigit(byte.is_ascii_hexdigit())
        .with_space(space)
        .with_print(graph || byte == b' ')
        .with_graph(graph)
        .with_blank(byte == b' ' || byte == b'\t')
        .with_cntrl(byte.is_ascii_control())
        .with_punct(graph && !alnum)
        .with_alnum(alnum)
        .raw_value()
}

const CTYPE_TABLE: [u16; CTYPE_TABLE_LENGTH] = {
    let mut table = [0u16; CTYPE_TABLE_LENGTH];
    let mut index = 0;
    while index < CTYPE_TABLE_LENGTH {
        let value = index as i32 - CTYPE_TABLE_INDEX_OF_ZERO as i32;
        if value >= 0 && value <= 0xff {
            table[index] = flags_for(value as u8);
        }
        index += 1;
    }
    table
};

static mut CTYPE_TABLE_POINTER: *const u16 = &CTYPE_TABLE[CTYPE_TABLE_INDEX_OF_ZERO] as *const u16;

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn __ctype_b_loc() -> *mut *const u16 {
    &raw mut CTYPE_TABLE_POINTER
}

// FIXME: only the printable ASCII range is classified as printable.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn iswprint(wide_character: c_uint) -> c_int {
    (wide_character >= 0x20 && wide_character <= 0x7e) as c_int
}
