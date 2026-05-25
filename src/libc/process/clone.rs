use std::{arch::asm, ffi::c_void, mem::size_of, os::fd::RawFd};

use bitbybit::bitfield;

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{exit, Syscall},
};

pub type ThreadID = i32;

#[bitfield(u64)]
pub struct Clone3Flags {
    #[bit(8, rw)]
    share_virtual_memory: bool,
    #[bit(9, rw)]
    share_filesystem_info: bool,
    #[bit(10, rw)]
    share_file_descriptors: bool,
    #[bit(11, rw)]
    share_signal_handlers: bool,
    #[bit(16, rw)]
    thread: bool,
    #[bit(18, rw)]
    share_sysvsem: bool,
    #[bit(19, rw)]
    set_tls: bool,
    #[bit(20, rw)]
    parent_set_tid: bool,
    #[bit(21, rw)]
    child_clear_tid: bool,
}

/// Kernel's `struct clone_args` — every field is u64-sized.
#[repr(C)]
pub struct Clone3Args {
    pub flags: Clone3Flags,
    pub pid_file_descriptor: *mut RawFd,
    pub child_tid_pointer: *mut ThreadID,
    pub parent_tid_pointer: *mut ThreadID,
    pub exit_signal: u64,
    pub child_stack: *mut u8,
    pub child_stack_size: u64,
    pub thread_local_storage: *mut u8,
    pub set_tid_array: *mut ThreadID,
    pub set_tid_array_count: u64,
    pub target_control_group: u64,
}

#[cfg_attr(not(test), no_mangle)]
pub unsafe extern "C" fn clone(
    entry_function: extern "C" fn(*mut c_void) -> i32,
    child_stack: *mut c_void,
    flags: i32,
    entry_argument: *mut c_void,
    mut varargs: ...
) -> i32 {
    signature_matches_libc!(libc::clone(
        entry_function,
        child_stack,
        flags,
        entry_argument
    ));

    if (entry_function as usize) == 0 || child_stack.is_null() {
        set_errno(Errno::INVAL);
        return -1;
    }

    let parent_tid_pointer: *mut ThreadID = varargs.arg();
    let thread_local_storage: *mut c_void = varargs.arg();
    let child_tid_pointer: *mut ThreadID = varargs.arg();

    let result: isize;
    asm!(
        "syscall",
        "test eax, eax",
        "jnz 2f",

        // child
        "mov rdi, {entry_function}",
        "mov rsi, {entry_argument}",
        "xor ebp, ebp",
        "call {clone_entry_point}",
        "ud2",

        // parent
        "2:",
        entry_function = in(reg) entry_function,
        entry_argument = in(reg) entry_argument,
        clone_entry_point = sym clone_entry_point_trampoline,
        inlateout("rax") Syscall::Clone as usize => result,
        in("rdi") flags as u32 as usize,
        in("rsi") ((child_stack as usize) & !0xF),
        in("rdx") parent_tid_pointer,
        in("r10") child_tid_pointer,
        in("r8") thread_local_storage,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack),
    );

    if result < 0 {
        set_errno(Errno((-result) as u32));
        return -1;
    }

    result as i32
}

unsafe fn clone3(
    args: *const Clone3Args,
    entry_function: unsafe extern "C" fn(*mut c_void) -> i32,
    entry_argument: *mut c_void,
) -> isize {
    let result: isize;
    asm!(
        "syscall",
        "test eax, eax",
        "jnz 2f",

        // child: pass the entry function and argument, then jump to the trampoline
        "mov rdi, {entry_function}",
        "mov rsi, {entry_argument}",
        "xor ebp, ebp",
        "call {clone_entry_point}",
        "ud2",

        // parent: result already in rax
        "2:",
        entry_function = in(reg) entry_function,
        entry_argument = in(reg) entry_argument,
        clone_entry_point = sym clone_entry_point_trampoline,
        inlateout("rax") Syscall::Clone3 as usize => result,
        in("rdi") args,
        in("rsi") size_of::<Clone3Args>(),
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack),
    );

    result
}

unsafe extern "C" fn clone_entry_point_trampoline(
    entry_function: unsafe extern "C" fn(*mut c_void) -> i32,
    entry_argument: *mut c_void,
) -> ! {
    let exit_code = entry_function(entry_argument);
    exit::exit(exit_code as usize);
}
