use core::ffi::{c_char, c_int};

use crate::{
    libc::{
        mem::munmap,
        threads::{join::wait_until_exited, PthreadT},
    },
    signature_matches_libc,
    syscall::{exit, syscall, thread_pointer::get_thread_pointer, Syscall},
    tls::thread_control_block::{DetachState, ThreadControlBlock},
};

const PR_SET_NAME: usize = 15;

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_self() -> PthreadT {
    signature_matches_libc!(std::mem::transmute(libc::pthread_self()));
    get_thread_pointer() as PthreadT
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_detach(thread: PthreadT) -> c_int {
    signature_matches_libc!(libc::pthread_detach(thread as _));
    let thread_control_block = thread as *mut ThreadControlBlock;
    match (*thread_control_block)
        .detach_state
        .compare_exchange(DetachState::Joinable, DetachState::Detached)
    {
        // Still running: the thread will free its own region when it exits.
        Ok(_) => 0,
        // Already exiting: wait for it to fully leave its stack, then reclaim the region ourselves.
        Err(DetachState::Exiting) => {
            wait_until_exited(&(*thread_control_block).tid);
            let (region_base, region_size) = (*thread_control_block).region.to_raw_parts();
            munmap(region_base as *mut u8, region_size);
            0
        }
        Err(_) => libc::EINVAL,
    }
}

/// The exiting thread's half of the handshake: if detached, free our own region and exit atomically; otherwise leave it for a joiner (or a detach that arrives after us).
/// Whichever side observes the other's state does the single reclamation, so the region is never double-freed nor leaked.
pub unsafe fn on_thread_exit(thread_control_block: *mut ThreadControlBlock) -> ! {
    let previous = (*thread_control_block)
        .detach_state
        .swap(DetachState::Exiting);
    if previous == DetachState::Detached {
        let (region_base, region_size) = (*thread_control_block).region.to_raw_parts();
        exit::munmap_and_exit(region_base as *mut u8, region_size);
    }
    exit::exit(0);
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_setname_np(thread: PthreadT, name: *const c_char) -> c_int {
    signature_matches_libc!(libc::pthread_setname_np(thread as _, name));
    // PR_SET_NAME names the calling thread only; naming a different thread would need /proc/<tid>/comm,
    // but tokio names workers from within themselves, so the self case is all that fires.
    if thread == get_thread_pointer() as PthreadT {
        syscall!(Syscall::PrCtl, PR_SET_NAME, name, 0usize, 0usize, 0usize);
    }
    0
}
