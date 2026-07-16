use core::{ffi::c_int, ptr::NonNull};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::{
    libc::threads::{futex_wait, futex_wake, mutex::PthreadMutex},
    signature_matches_libc,
};

/// A sequence-counter condvar. `sequence` is bumped by every signal/broadcast;
/// a waiter snapshots it under the mutex, so any signal after the snapshot moves it off the waited value — no lost wakeup.
#[repr(C, align(8))]
struct PthreadCond {
    sequence: AtomicU32,
    waiters: AtomicU32,
    _reserved: [u8; 40],
}

const _: () = assert!(size_of::<PthreadCond>() == size_of::<libc::pthread_cond_t>());
const _: () = assert!(align_of::<PthreadCond>() == align_of::<libc::pthread_cond_t>());

impl PthreadCond {
    const fn new() -> Self {
        Self {
            sequence: AtomicU32::new(0),
            waiters: AtomicU32::new(0),
            _reserved: [0; 40],
        }
    }

    fn wake(&self, count: i32) {
        self.sequence.fetch_add(1, Ordering::Release);
        if self.waiters.load(Ordering::Relaxed) != 0 {
            futex_wake(&self.sequence, count);
        }
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_cond_wait(cond: &PthreadCond, mutex: &PthreadMutex) -> c_int {
    signature_matches_libc!(libc::pthread_cond_wait(
        std::mem::transmute(cond),
        std::mem::transmute(mutex)
    ));

    // Snapshot the sequence while the mutex is still held: a signal can only bump it after we release below.
    let observed = cond.sequence.load(Ordering::Acquire);
    cond.waiters.fetch_add(1, Ordering::Relaxed);

    mutex.release_for_wait();
    futex_wait(&cond.sequence, observed);
    cond.waiters.fetch_sub(1, Ordering::Relaxed);
    mutex.acquire_after_wait();
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_cond_signal(cond: &PthreadCond) -> c_int {
    signature_matches_libc!(libc::pthread_cond_signal(std::mem::transmute(cond)));
    cond.wake(1);
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_cond_broadcast(cond: &PthreadCond) -> c_int {
    signature_matches_libc!(libc::pthread_cond_broadcast(std::mem::transmute(cond)));
    cond.wake(i32::MAX);
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_cond_init(
    cond: &mut PthreadCond,
    _attr: Option<NonNull<libc::pthread_condattr_t>>,
) -> c_int {
    signature_matches_libc!(libc::pthread_cond_init(
        std::mem::transmute(cond),
        std::mem::transmute(_attr)
    ));
    *cond = PthreadCond::new();
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_cond_destroy(cond: &PthreadCond) -> c_int {
    signature_matches_libc!(libc::pthread_cond_destroy(std::mem::transmute(cond)));
    0
}
