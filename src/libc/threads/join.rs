use std::{
    ffi::c_void,
    ptr::{self, NonNull},
};

use crate::{
    libc::mem::munmap,
    signature_matches_libc,
    syscall::{syscall, Syscall, FUTEX_WAIT},
    tls::thread_control_block::ThreadControlBlock,
};

type PthreadT = usize;

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_join(thread_addr: PthreadT, return_value: *mut *mut c_void) -> i32 {
    signature_matches_libc!(libc::pthread_join(thread_addr as _, return_value));

    let thread_control_block = thread_addr as *const ThreadControlBlock;
    let tid_pointer = ptr::addr_of!((*thread_control_block).tid);

    loop {
        let current_tid = ptr::read_volatile(tid_pointer);
        if current_tid == 0 {
            break;
        }
        syscall!(
            Syscall::Futex,
            tid_pointer,
            FUTEX_WAIT,
            current_tid as usize,
            0usize,
            0usize,
            0usize
        );
    }

    if let Some(return_value) = NonNull::new(return_value) {
        *return_value.as_ptr() = (*thread_control_block).return_value;
    }

    let region = (*thread_control_block).region;
    let (region_base, region_size) = region.to_raw_parts();
    munmap(region_base as *mut u8, region_size);

    0
}
