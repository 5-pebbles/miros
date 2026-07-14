pub mod printf;

use core::ffi::{c_char, c_int, c_void};
use std::ptr;

use crate::signature_matches_libc;

/// Yields the bytes of a null-terminated string, stopping before the terminator.
unsafe fn c_string_bytes(start: *const c_char) -> impl Iterator<Item = u8> {
    (0..)
        .map(move |offset| *start.add(offset) as u8)
        .take_while(|&byte| byte != 0)
}

/// A 256-entry membership table for the bytes of a null-terminated set (terminator excluded).
unsafe fn byte_membership(characters: *const c_char) -> [bool; 256] {
    c_string_bytes(characters).fold([false; 256], |mut set, byte| {
        set[byte as usize] = true;
        set
    })
}

/// The C-ordering value at the first differing byte over `offsets`, or `None` if equal throughout.
unsafe fn first_byte_difference(
    left: *const c_char,
    right: *const c_char,
    offsets: impl Iterator<Item = usize>,
) -> Option<c_int> {
    offsets
        .map(|offset| (*left.add(offset) as u8, *right.add(offset) as u8))
        .find_map(|(left_byte, right_byte)| {
            (left_byte != right_byte || left_byte == 0)
                .then_some(left_byte as c_int - right_byte as c_int)
        })
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn strlen(start_character: *mut i8) -> usize {
    signature_matches_libc!(libc::strlen(start_character));
    c_string_bytes(start_character).count()
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn strcmp(left: *const c_char, right: *const c_char) -> c_int {
    signature_matches_libc!(libc::strcmp(left, right));
    first_byte_difference(left, right, 0..).unwrap_unchecked()
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn strncmp(left: *const c_char, right: *const c_char, length: usize) -> c_int {
    signature_matches_libc!(libc::strncmp(left, right, length));
    first_byte_difference(left, right, 0..length).unwrap_or(0)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn strcpy(destination: *mut c_char, source: *const c_char) -> *mut c_char {
    signature_matches_libc!(libc::strcpy(destination, source));
    c_string_bytes(source)
        .chain([0])
        .enumerate()
        .for_each(|(offset, byte)| *destination.add(offset) = byte as c_char);
    destination
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn strncpy(
    destination: *mut c_char,
    source: *const c_char,
    length: usize,
) -> *mut c_char {
    signature_matches_libc!(libc::strncpy(destination, source, length));
    // Exactly `length` bytes: the string, then a terminator, then zero-padding — all truncated.
    c_string_bytes(source)
        .chain(std::iter::repeat(0))
        .take(length)
        .enumerate()
        .for_each(|(offset, byte)| *destination.add(offset) = byte as c_char);
    destination
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn strchr(string: *const c_char, character: c_int) -> *mut c_char {
    signature_matches_libc!(libc::strchr(string, character));
    let needle = character as u8;
    (0..)
        .find_map(|offset| match *string.add(offset) as u8 {
            byte if byte == needle => Some(string.add(offset) as *mut c_char),
            0 => Some(ptr::null_mut()),
            _ => None,
        })
        .unwrap_unchecked()
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn strrchr(string: *const c_char, character: c_int) -> *mut c_char {
    signature_matches_libc!(libc::strrchr(string, character));
    let needle = character as u8;
    c_string_bytes(string)
        .chain([0])
        .enumerate()
        .filter(|&(_, byte)| byte == needle)
        .last()
        .map(|(offset, _)| string.add(offset) as *mut c_char)
        .unwrap_or(ptr::null_mut())
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn strspn(string: *const c_char, accept: *const c_char) -> usize {
    signature_matches_libc!(libc::strspn(string, accept));
    let accepted = byte_membership(accept);
    c_string_bytes(string)
        .take_while(|&byte| accepted[byte as usize])
        .count()
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn strcspn(string: *const c_char, reject: *const c_char) -> usize {
    signature_matches_libc!(libc::strcspn(string, reject));
    let rejected = byte_membership(reject);
    c_string_bytes(string)
        .take_while(|&byte| !rejected[byte as usize])
        .count()
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn memchr(memory: *const c_void, character: c_int, length: usize) -> *mut c_void {
    signature_matches_libc!(libc::memchr(memory, character, length));
    let needle = character as u8;
    let bytes = memory.cast::<u8>();
    (0..length)
        .find(|&offset| *bytes.add(offset) == needle)
        .map(|offset| bytes.add(offset) as *mut c_void)
        .unwrap_or(ptr::null_mut())
}
