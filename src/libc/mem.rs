use std::arch::asm;

use crate::signature_matches_libc;

#[no_mangle]
unsafe extern "C" fn memcpy(
    destination: *mut u8,
    source: *const u8,
    number_of_bytes_to_copy: usize,
) -> *mut u8 {
    signature_matches_libc!(core::mem::transmute(libc::memcpy(
        core::mem::transmute(destination),
        core::mem::transmute(source),
        core::mem::transmute(number_of_bytes_to_copy)
    )));
    asm!(
        "rep movsb",
        inout("rdi") destination => _,
        inout("rsi") source => _,
        inout("rcx") number_of_bytes_to_copy => _,
        options(nostack, preserves_flags)
    );
    destination
}

#[no_mangle]
unsafe extern "C" fn memset(
    destination: *mut u8,
    single_byte_thats_32_bits_for_some_fucking_reason: u32, // I hate this stupid fucking API... Like why?
    number_of_bytes_to_set: usize,
) -> *mut u8 {
    // SAFETY: Yes, I know...
    let byte = single_byte_thats_32_bits_for_some_fucking_reason as u8;
    asm!(
        "rep stosb",
        inout("rdi") destination => _,
        in("al") byte,
        inout("rcx") number_of_bytes_to_set => _,
        options(nostack, preserves_flags)

    );
    destination
}

#[no_mangle]
unsafe extern "C" fn memcmp(
    left_pointer: *const u8,
    right_pointer: *const u8,
    length_of_comparison: usize,
) -> i32 {
    let ordering: i32;
    asm!(
        "repe cmpsb",
        "seta {ordering:l}",
        "sbb {ordering:e}, 0",
        inout("rdi") left_pointer => _,
        inout("rsi") right_pointer => _,
        inout("rcx") length_of_comparison => _,
        ordering = out(reg) ordering,
        options(nostack)
    );
    ordering
}
