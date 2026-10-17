use core::ffi::{c_char, c_int, c_void};

// FIXME: the C locale encodes one byte per character.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn __ctype_get_mb_cur_max() -> usize {
    1
}

// The C locale is stateless, so every state object counts as initial.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn mbsinit(_state: *const c_void) -> c_int {
    1
}

// FIXME: the C locale maps each byte to one `char32_t`; `(size_t)-2` means the input was too short.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn mbrtoc32(
    codepoint: *mut u32,
    input: *const c_char,
    length: usize,
    _state: *mut c_void,
) -> usize {
    if input.is_null() {
        return 0;
    }
    if length == 0 {
        return usize::MAX - 1;
    }

    let byte = *(input as *const u8);
    if !codepoint.is_null() {
        *codepoint = byte as u32;
    }

    if byte == 0 {
        0
    } else {
        1
    }
}
