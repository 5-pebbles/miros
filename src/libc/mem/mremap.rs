use bitbybit::bitfield;

use crate::{
    io_macros::syscall_debug_assert,
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[bitfield(u32)]
pub struct MremapFlags {
    #[bit(0, rw)]
    may_move: bool,
    #[bit(1, rw)]
    fixed: bool,
    #[bit(2, rw)]
    dont_unmap: bool,
}

#[cfg_attr(not(test), no_mangle)]
pub unsafe extern "C" fn mremap(
    old_address: *mut u8,
    old_size: usize,
    new_size: usize,
    flags: MremapFlags,
    mut args: ...
) -> *mut u8 {
    signature_matches_libc!(libc::mremap(
        old_address.cast(),
        old_size,
        new_size,
        std::mem::transmute(flags)
    )
    .cast());

    let new_address = if flags.fixed() {
        args.arg::<*mut u8>()
    } else {
        std::ptr::null_mut()
    };

    let result = syscall!(
        Syscall::MreMap,
        old_address,
        old_size,
        new_size,
        flags.raw_value(),
        new_address
    );
    syscall_debug_assert!(result >= 0);
    result as *mut u8
}
