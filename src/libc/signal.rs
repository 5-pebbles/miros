use std::{
    arch::naked_asm,
    ffi::{c_int, c_ulong},
    mem::{size_of, transmute, zeroed},
    ptr,
};

use crate::{libc::translate_syscall_result, signature_matches_libc, syscall, syscall::Syscall};

// x86_64 has no signal-return trampoline, so SA_RESTORER must name ours.
const SA_RESTORER: c_ulong = 0x0400_0000;

// glibc's sigset_t is 128 bytes; the kernel only reads the low word.
const KERNEL_SIGSET_SIZE: usize = 8;

// The kernel's struct kernel_sigaction: flags, restorer, and mask word inline.
#[repr(C)]
struct KernelSigaction {
    handler: usize,
    flags: c_ulong,
    restorer: usize,
    mask: u64,
}

// The kernel resumes here after the handler; rt_sigreturn never returns.
#[unsafe(naked)]
unsafe extern "C" fn sigreturn_trampoline() {
    naked_asm!(
        "mov rax, {number}",
        "syscall",
        number = const Syscall::RtSigReturn as usize,
    )
}

unsafe fn sigset_low_word(mask: *const libc::sigset_t) -> u64 {
    ptr::read_unaligned(mask.cast())
}

unsafe fn install(
    signal_number: c_int,
    action: *const libc::sigaction,
    old_action: *mut libc::sigaction,
) -> c_int {
    let kernel_new = action.as_ref().map(|source| KernelSigaction {
        handler: source.sa_sigaction,
        flags: source.sa_flags as c_ulong | SA_RESTORER,
        restorer: sigreturn_trampoline as *const () as usize,
        mask: sigset_low_word(&source.sa_mask),
    });
    let kernel_new_pointer = kernel_new.as_ref().map_or(ptr::null(), ptr::from_ref);

    let mut kernel_old = KernelSigaction {
        handler: 0,
        flags: 0,
        restorer: 0,
        mask: 0,
    };
    let kernel_old_pointer = if old_action.is_null() {
        ptr::null_mut()
    } else {
        &mut kernel_old
    };

    let result = translate_syscall_result(syscall!(
        Syscall::RtSigAction,
        signal_number,
        kernel_new_pointer,
        kernel_old_pointer,
        KERNEL_SIGSET_SIZE
    ));
    if result < 0 {
        return -1;
    }

    if let Some(destination) = old_action.as_mut() {
        destination.sa_sigaction = kernel_old.handler;
        destination.sa_flags = kernel_old.flags as c_int;
        let restorer = transmute::<usize, Option<extern "C" fn()>>(kernel_old.restorer);
        destination.sa_restorer = restorer;
        let mask_pointer = &mut destination.sa_mask as *mut libc::sigset_t;
        ptr::write_bytes(mask_pointer.cast::<u8>(), 0, size_of::<libc::sigset_t>());
        ptr::write_unaligned(mask_pointer.cast::<u64>(), kernel_old.mask);
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
    // BSD semantics: SA_RESTART, report the prior handler.
    let action = libc::sigaction {
        sa_sigaction: handler,
        sa_mask: zeroed(),
        sa_flags: libc::SA_RESTART,
        sa_restorer: None,
    };
    let mut previous = zeroed();
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
    // glibc's stack_t matches the kernel layout, so pointers pass straight through.
    translate_syscall_result(syscall!(Syscall::SigAltStack, new_stack, old_stack)) as c_int
}

// No realtime signals are reserved, so userspace gets the kernel's full range.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn __libc_current_sigrtmax() -> c_int {
    signature_matches_libc!(libc::__libc_current_sigrtmax());
    64
}
