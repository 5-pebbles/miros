use core::{ffi::c_int, ptr::NonNull};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::{
    libc::threads::{futex_wait, futex_wake},
    signature_matches_libc,
};

/// `state` is both the lock word and the futex: top bit = write-held, low 31 bits = active readers.
/// Waiters park on the exact value they read, so `futex_wait`'s atomic compare closes every wakeup race;
/// the cost is spurious wakeups (a release wakes all, losers re-sleep), which is fine for a cold lock.
const WRITE_HELD: u32 = 1 << 31;

#[repr(C, align(8))]
struct PthreadRwlock {
    state: AtomicU32,
    _reserved: [u8; 52],
}

const _: () = assert!(size_of::<PthreadRwlock>() == size_of::<libc::pthread_rwlock_t>());
const _: () = assert!(align_of::<PthreadRwlock>() == align_of::<libc::pthread_rwlock_t>());

impl PthreadRwlock {
    const fn new() -> Self {
        Self {
            state: AtomicU32::new(0),
            _reserved: [0; 52],
        }
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlock_rdlock(rwlock: &PthreadRwlock) -> c_int {
    signature_matches_libc!(libc::pthread_rwlock_rdlock(std::mem::transmute(rwlock)));
    loop {
        let current = rwlock.state.load(Ordering::Acquire);
        if current & WRITE_HELD != 0 {
            futex_wait(&rwlock.state, current);
        } else if rwlock
            .state
            .compare_exchange_weak(current, current + 1, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return 0;
        }
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlock_tryrdlock(rwlock: &PthreadRwlock) -> c_int {
    signature_matches_libc!(libc::pthread_rwlock_tryrdlock(std::mem::transmute(rwlock)));
    let current = rwlock.state.load(Ordering::Acquire);
    if current & WRITE_HELD != 0 {
        return libc::EBUSY;
    }
    if rwlock
        .state
        .compare_exchange(current, current + 1, Ordering::Acquire, Ordering::Relaxed)
        .is_ok()
    {
        0
    } else {
        libc::EBUSY
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlock_wrlock(rwlock: &PthreadRwlock) -> c_int {
    signature_matches_libc!(libc::pthread_rwlock_wrlock(std::mem::transmute(rwlock)));
    loop {
        let current = rwlock.state.load(Ordering::Acquire);
        if current != 0 {
            futex_wait(&rwlock.state, current);
        } else if rwlock
            .state
            .compare_exchange_weak(0, WRITE_HELD, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return 0;
        }
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlock_trywrlock(rwlock: &PthreadRwlock) -> c_int {
    signature_matches_libc!(libc::pthread_rwlock_trywrlock(std::mem::transmute(rwlock)));
    if rwlock
        .state
        .compare_exchange(0, WRITE_HELD, Ordering::Acquire, Ordering::Relaxed)
        .is_ok()
    {
        0
    } else {
        libc::EBUSY
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlock_unlock(rwlock: &PthreadRwlock) -> c_int {
    signature_matches_libc!(libc::pthread_rwlock_unlock(std::mem::transmute(rwlock)));
    if rwlock.state.load(Ordering::Relaxed) & WRITE_HELD != 0 {
        rwlock.state.store(0, Ordering::Release);
        futex_wake(&rwlock.state, i32::MAX);
    } else if rwlock.state.fetch_sub(1, Ordering::Release) == 1 {
        // Last reader out: wake whoever is waiting for a clear lock.
        futex_wake(&rwlock.state, i32::MAX);
    }
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlock_init(
    rwlock: &mut PthreadRwlock,
    _attr: Option<NonNull<libc::pthread_rwlockattr_t>>,
) -> c_int {
    signature_matches_libc!(libc::pthread_rwlock_init(
        std::mem::transmute(rwlock),
        std::mem::transmute(_attr)
    ));
    *rwlock = PthreadRwlock::new();
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlock_destroy(rwlock: &PthreadRwlock) -> c_int {
    signature_matches_libc!(libc::pthread_rwlock_destroy(std::mem::transmute(rwlock)));
    0
}
