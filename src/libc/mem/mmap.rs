use std::os::fd::RawFd;

use crate::{
    io_macros::syscall_debug_assert,
    libc::mem::{MapFlags, ProtectionFlags},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

// TODO: add error handling
#[cfg_attr(not(test), no_mangle)]
pub unsafe extern "C" fn mmap(
    pointer: *mut u8,
    size: usize,
    protection_flags: ProtectionFlags,
    map_flags: MapFlags,
    file_descriptor: RawFd,
    file_offset: usize,
) -> *mut u8 {
    signature_matches_libc!((libc::mmap(
        pointer.cast(),
        size,
        std::mem::transmute(protection_flags),
        std::mem::transmute(map_flags),
        file_descriptor,
        file_offset as i64,
    ))
    .cast());
    let result = syscall!(
        Syscall::MMap,
        pointer,
        size,
        protection_flags.raw_value(),
        map_flags.raw_value(),
        file_descriptor,
        file_offset
    );
    syscall_debug_assert!(result >= 0);
    result as *mut u8
}
