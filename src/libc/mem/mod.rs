// TODO: extract functions into files
use std::arch::asm;

use bitbybit::{bitenum, bitfield};

use crate::{io_macros::syscall_debug_assert, signature_matches_libc, syscall::Syscall};

mod mmap;
pub use mmap::mmap;

// Protection flags:
#[bitenum(u2, exhaustive = true)]
pub enum GrowthDirection {
    FixedSize = 0b00,
    GrowsDown = 0b01,
    GrowsUp = 0b10,
    Invalid = 0b11,
}

#[bitfield(u32)]
pub struct ProtectionFlags {
    #[bit(0, rw)]
    readable: bool,
    #[bit(1, rw)]
    writable: bool,
    #[bit(2, rw)]
    executable: bool,
    #[bits(24..=25, rw)]
    growth_direction: GrowthDirection,
}

// MAP flags:
#[bitfield(u32)]
pub struct MapFlags {
    #[bit(0, rw)]
    shared: bool,
    #[bit(1, rw)]
    private: bool,
    #[bit(4, rw)]
    fixed: bool,
    #[bit(5, rw)]
    anonymous: bool,
}

// TODO: add error handling
#[cfg_attr(not(test), no_mangle)]
pub unsafe fn munmap(pointer: *mut u8, size: usize) -> i32 {
    signature_matches_libc!(libc::munmap(pointer.cast(), size));

    let mut result: isize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") Syscall::Munmap as usize => result,
            in("rdi") pointer,
            in("rsi") size,
            out("rcx") _,
            out("r11") _,
            options(nostack)
        )
    };
    syscall_debug_assert!(result >= 0);
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn memcpy(
    destination: *mut u8,
    source: *const u8,
    number_of_bytes_to_copy: usize,
) -> *mut u8 {
    signature_matches_libc!(std::mem::transmute(libc::memcpy(
        std::mem::transmute(destination),
        std::mem::transmute(source),
        std::mem::transmute(number_of_bytes_to_copy)
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

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn memset(
    destination: *mut u8,
    single_byte_thats_32_bits_for_some_fucking_reason: u32, // I hate this stupid fucking API... Like why?
    number_of_bytes_to_set: usize,
) -> *mut u8 {
    signature_matches_libc!(std::mem::transmute(libc::memset(
        destination.cast(),
        single_byte_thats_32_bits_for_some_fucking_reason as i32,
        number_of_bytes_to_set
    )));

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

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn memcmp(
    left_pointer: *const u8,
    right_pointer: *const u8,
    length_of_comparison: usize,
) -> i32 {
    signature_matches_libc!(libc::memcmp(
        left_pointer.cast(),
        right_pointer.cast(),
        length_of_comparison
    ));

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
