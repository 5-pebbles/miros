use std::{arch::asm, ffi::c_void};

use bitbybit::{bitenum, bitfield};

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::Syscall,
};

#[bitenum(u2, exhaustive = true)]
#[derive(Default)]
enum EntropySource {
    #[default]
    DevSlashUrandom = 0b00,
    DevSlashRandom = 0b01,
    Insecure = 0b10,
    Invalid = 0b11,
}

#[bitfield(u32)]
struct GetRandomFlags {
    #[bit(0, rw)]
    non_blocking: bool,
    #[bits(1..=2, rw)]
    entropy_source: EntropySource,
}

#[no_mangle]
unsafe extern "C" fn getrandom(
    buffer_pointer: *mut c_void,
    buffer_length_in_bytes: usize,
    flags: GetRandomFlags,
) -> isize {
    signature_matches_libc!(libc::getrandom(
        buffer_pointer,
        buffer_length_in_bytes,
        std::mem::transmute(flags),
    ));

    let result: isize;

    #[cfg(target_arch = "x86_64")]
    {
        asm!(
            "syscall",
            inlateout("rax") Syscall::GetRandom as usize => result,
            in("rdi") buffer_pointer,
            in("rsi") buffer_length_in_bytes,
            in("rdx") flags.raw_value(),
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags)
        );
    }

    if result < 0 {
        set_errno(Errno(result.abs() as u32));
        -1
    } else {
        result
    }
}
