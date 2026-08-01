use core::{
    ffi::{c_char, c_int},
    ptr,
};

use crate::{
    libc::{
        mem::munmap,
        str::strlen,
        threads::{join::wait_until_exited, PthreadT},
    },
    signature_matches_libc,
    syscall::{exit, syscall, thread_pointer::get_thread_pointer, Syscall},
    tls::thread_control_block::{DetachState, ThreadControlBlock},
};

const PR_SET_NAME: usize = 15;
const TASK_COMM_LEN: usize = 16;

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
        // Still running: the thread frees its own region when it exits.
        Ok(_) => 0,
        // Already exiting: wait for it to fully leave its stack, then reclaim the region ourselves.
        Err(DetachState::Exiting) => {
            wait_until_exited(ptr::addr_of!((*thread_control_block).tid));
            let (region_base, region_size) = (*thread_control_block).region.to_raw_parts();
            munmap(region_base as *mut u8, region_size);
            0
        }
        Err(_) => libc::EINVAL,
    }
}

/// Only called on the exiting thread.
pub unsafe fn on_thread_exit(thread_control_block: *mut ThreadControlBlock) -> ! {
    let previous = (*thread_control_block)
        .detach_state
        .swap(DetachState::Exiting);
    // If detached, munmap our own region and exit without touching the stack in between.
    // Otherwise leave the region for a joiner (or a detach that arrives after us).
    if previous == DetachState::Detached {
        let (region_base, region_size) = (*thread_control_block).region.to_raw_parts();
        exit::munmap_and_exit(region_base as *mut u8, region_size);
    }
    exit::exit(0);
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_setname_np(thread: PthreadT, name: *const c_char) -> c_int {
    signature_matches_libc!(libc::pthread_setname_np(thread as _, name));

    let name_length = strlen(name as *mut _);
    if name_length >= TASK_COMM_LEN {
        return libc::ERANGE;
    }

    if thread == get_thread_pointer() as PthreadT {
        let result = syscall!(Syscall::PrCtl, PR_SET_NAME, name, 0usize, 0usize, 0usize);
        return if result < 0 { (-result) as c_int } else { 0 };
    }

    // prctl names only the caller; other threads go through their comm file, as glibc does.
    let tid = (*(thread as *const ThreadControlBlock)).tid;
    let path = task_comm_path(tid);
    let file_descriptor = syscall!(Syscall::OpenAt, 0usize, path.as_ptr(), libc::O_RDWR, 0);
    if file_descriptor < 0 {
        return (-file_descriptor) as c_int;
    }

    let written = loop {
        let result = syscall!(Syscall::Write, file_descriptor, name, name_length);
        if result != -(libc::EINTR as isize) {
            break result;
        }
    };
    syscall!(Syscall::Close, file_descriptor);

    if written < 0 {
        (-written) as c_int
    } else if written as usize != name_length {
        libc::EIO
    } else {
        0
    }
}

/// Builds the NUL-terminated `/proc/self/task/<tid>/comm` path; the buffer fits a 10-digit tid exactly.
fn task_comm_path(tid: i32) -> [u8; 32] {
    let mut path = [0u8; 32];
    let prefix = b"/proc/self/task/";
    path[..prefix.len()].copy_from_slice(prefix);

    let mut value = tid as u32;
    let mut cursor = prefix.len();
    loop {
        *path.get_mut(cursor).unwrap() = b'0' + (value % 10) as u8;
        cursor += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    path[prefix.len()..cursor].reverse();

    let suffix = b"/comm";
    path[cursor..cursor + suffix.len()].copy_from_slice(suffix);
    path
}
