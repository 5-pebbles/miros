use core::ffi::{c_int, c_ulong};
use std::{arch::naked_asm, mem::size_of, ptr};

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

/// x86_64 ships no kernel signal-return trampoline; `sa_restorer` must point at userspace code that calls `rt_sigreturn`, and this flag tells the kernel the field is populated.
const SA_RESTORER: c_ulong = 0x0400_0000;

/// glibc's `sigset_t` is 128 bytes; the kernel's is 8 (`_NSIG / 8`), and only that low word crosses the syscall boundary.
const KERNEL_SIGSET_SIZE: usize = 8;

/// The kernel's `struct kernel_sigaction`: flags and restorer precede a single-word mask, unlike glibc's userspace `struct sigaction`.
#[repr(C)]
struct KernelSigaction {
    handler: usize,
    flags: c_ulong,
    restorer: usize,
    mask: u64,
}

/// The kernel transfers control here after a handler returns; `rt_sigreturn` restores the interrupted context and never comes back.
#[unsafe(naked)]
unsafe extern "C" fn sigreturn_trampoline() {
    naked_asm!(
        "mov rax, {number}",
        "syscall",
        number = const Syscall::RtSigReturn as usize,
    )
}

/// The low 64 bits of a glibc `sigset_t` — all the kernel consumes.
unsafe fn sigset_low_word(mask: *const libc::sigset_t) -> u64 {
    ptr::read_unaligned(mask.cast())
}

/// Translate glibc → kernel on the way in, kernel → glibc on the way out; shared by `sigaction` and `signal`.
unsafe fn install(
    signal_number: c_int,
    action: *const libc::sigaction,
    old_action: *mut libc::sigaction,
) -> c_int {
    let mut kernel_new = KernelSigaction {
        handler: 0,
        flags: 0,
        restorer: 0,
        mask: 0,
    };
    let kernel_new_pointer = action
        .as_ref()
        .map(|source| {
            kernel_new = KernelSigaction {
                handler: source.sa_sigaction,
                flags: source.sa_flags as c_ulong | SA_RESTORER,
                restorer: sigreturn_trampoline as *const () as usize,
                mask: sigset_low_word(&source.sa_mask),
            };
            &kernel_new as *const KernelSigaction
        })
        .unwrap_or(ptr::null());

    let mut kernel_old = KernelSigaction {
        handler: 0,
        flags: 0,
        restorer: 0,
        mask: 0,
    };
    let kernel_old_pointer = old_action
        .is_null()
        .then_some(ptr::null_mut())
        .unwrap_or(&mut kernel_old);

    let result = syscall!(
        Syscall::RtSigAction,
        signal_number,
        kernel_new_pointer,
        kernel_old_pointer,
        KERNEL_SIGSET_SIZE
    );
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        return -1;
    }

    if let Some(destination) = old_action.as_mut() {
        destination.sa_sigaction = kernel_old.handler;
        destination.sa_flags = kernel_old.flags as c_int;
        destination.sa_restorer =
            core::mem::transmute::<usize, Option<extern "C" fn()>>(kernel_old.restorer);
        ptr::write_bytes(
            &mut destination.sa_mask as *mut _ as *mut u8,
            0,
            size_of::<libc::sigset_t>(),
        );
        ptr::write_unaligned(
            &mut destination.sa_mask as *mut _ as *mut u64,
            kernel_old.mask,
        );
    }
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn sigaction(
    signal_number: c_int,
    action: *const libc::sigaction,
    old_action: *mut libc::sigaction,
) -> c_int {
    signature_matches_libc!(libc::sigaction(signal_number, action, old_action));
    install(signal_number, action, old_action)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn signal(
    signal_number: c_int,
    handler: libc::sighandler_t,
) -> libc::sighandler_t {
    signature_matches_libc!(libc::signal(signal_number, handler));
    // BSD `signal`: install with SA_RESTART and report the prior handler.
    let action = libc::sigaction {
        sa_sigaction: handler,
        sa_mask: core::mem::zeroed(),
        sa_flags: libc::SA_RESTART,
        sa_restorer: None,
    };
    let mut previous: libc::sigaction = core::mem::zeroed();
    if install(signal_number, &action, &mut previous) < 0 {
        return libc::SIG_ERR;
    }
    previous.sa_sigaction
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn sigaltstack(
    new_stack: *const libc::stack_t,
    old_stack: *mut libc::stack_t,
) -> c_int {
    signature_matches_libc!(libc::sigaltstack(new_stack, old_stack));
    // Kernel `stack_t` matches glibc's, so the pointers pass straight through.
    let result = syscall!(Syscall::SigAltStack, new_stack, old_stack);
    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        0
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn __libc_current_sigrtmax() -> c_int {
    signature_matches_libc!(libc::__libc_current_sigrtmax());
    64
}
