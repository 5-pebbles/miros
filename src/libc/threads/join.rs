use std::{
    ffi::c_void,
    ptr::{self, NonNull},
};

use crate::{
    libc::{mem::munmap, threads::PthreadT},
    signature_matches_libc,
    syscall::{futex::FutexOperation, syscall, Syscall},
    tls::thread_control_block::ThreadControlBlock,
};

/// Block until the kernel clears `tid` — set via `CLONE_CHILD_CLEARTID` as the last act of a dying thread,
/// so `tid == 0` means the thread is fully off its stack and its region is safe to unmap.
pub unsafe fn wait_until_exited(tid_pointer: *const i32) {
    loop {
        let current_tid = ptr::read_volatile(tid_pointer);
        if current_tid == 0 {
            break;
        }
        syscall!(
            Syscall::Futex,
            tid_pointer,
            FutexOperation::Wait,
            current_tid as usize,
            0usize,
            0usize,
            0usize
        );
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_join(
    thread_addr: PthreadT,
    return_value: Option<NonNull<*mut c_void>>,
) -> i32 {
    signature_matches_libc!(libc::pthread_join(
        thread_addr as _,
        std::mem::transmute(return_value)
    ));

    let thread_control_block = thread_addr as *const ThreadControlBlock;
    wait_until_exited(ptr::addr_of!((*thread_control_block).tid));

    if let Some(return_value) = return_value {
        *return_value.as_ptr() = (*thread_control_block).return_value;
    }

    let region = (*thread_control_block).region;
    let (region_base, region_size) = region.to_raw_parts();
    munmap(region_base as *mut u8, region_size);

    0
}
