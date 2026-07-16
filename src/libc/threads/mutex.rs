use core::{ffi::c_int, ptr::NonNull};
use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering},
};

use strum::FromRepr;

use crate::{
    libc::threads::{current_tid, futex_wait, futex_wake},
    signature_matches_libc,
};

const UNLOCKED: u32 = 0;
const LOCKED: u32 = 1;
const CONTENDED: u32 = 2;

#[derive(FromRepr, PartialEq, Clone, Copy)]
#[repr(i32)]
enum MutexKind {
    Normal = 0,
    Recursive = 1,
    ErrorCheck = 2,
}

/// Field offsets mirror glibc's `pthread_mutex_t.__data`, so a static `PTHREAD_*_MUTEX_INITIALIZER` blob (which writes `__kind`/`__owner`/`__count` at these offsets) is a valid mutex without `_init`.
/// `state` (glibc's `__lock`) uses a Drepper 3-state scheme; only our own code ever reads it.
#[repr(C, align(8))]
pub struct PthreadMutex {
    state: AtomicU32,
    recursion: UnsafeCell<u32>,
    owner: AtomicU32,
    _nusers: u32,
    kind: i32,
    _reserved: [u8; 20],
}

const _: () = assert!(size_of::<PthreadMutex>() == size_of::<libc::pthread_mutex_t>());
const _: () = assert!(align_of::<PthreadMutex>() == align_of::<libc::pthread_mutex_t>());

// SAFETY: `recursion` is only touched by the thread whose tid is in `owner`.
unsafe impl Sync for PthreadMutex {}

impl PthreadMutex {
    const fn new(kind: i32) -> Self {
        Self {
            state: AtomicU32::new(UNLOCKED),
            recursion: UnsafeCell::new(0),
            owner: AtomicU32::new(0),
            _nusers: 0,
            kind,
            _reserved: [0; 20],
        }
    }

    fn kind(&self) -> MutexKind {
        MutexKind::from_repr(self.kind).unwrap_or(MutexKind::Normal)
    }

    /// Drepper mutex2 acquire: fast CAS `0 -> 1`, else park on state `2` until it drops to `0`.
    fn acquire(&self) {
        if self
            .state
            .compare_exchange(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return;
        }
        while self.state.swap(CONTENDED, Ordering::Acquire) != UNLOCKED {
            futex_wait(&self.state, CONTENDED);
        }
    }

    fn try_acquire(&self) -> bool {
        self.state
            .compare_exchange(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    /// Release; wake one waiter only if the word was contended.
    fn release(&self) {
        if self.state.swap(UNLOCKED, Ordering::Release) == CONTENDED {
            futex_wake(&self.state, 1);
        }
    }

    unsafe fn acquire_owned(&self, tid: u32) {
        self.acquire();
        self.owner.store(tid, Ordering::Relaxed);
        *self.recursion.get() = 1;
    }

    /// Fully release for `pthread_cond_wait`, dropping ownership regardless of kind.
    pub unsafe fn release_for_wait(&self) {
        if self.kind() != MutexKind::Normal {
            self.owner.store(0, Ordering::Relaxed);
            *self.recursion.get() = 0;
        }
        self.release();
    }

    /// Re-acquire after `pthread_cond_wait` returns.
    pub unsafe fn acquire_after_wait(&self) {
        match self.kind() {
            MutexKind::Normal => self.acquire(),
            _ => self.acquire_owned(current_tid()),
        }
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_mutex_lock(mutex: &PthreadMutex) -> c_int {
    signature_matches_libc!(libc::pthread_mutex_lock(std::mem::transmute(mutex)));
    match mutex.kind() {
        MutexKind::Normal => mutex.acquire(),
        MutexKind::Recursive => {
            let tid = current_tid();
            if mutex.owner.load(Ordering::Relaxed) == tid {
                *mutex.recursion.get() += 1;
                return 0;
            }
            mutex.acquire_owned(tid);
        }
        MutexKind::ErrorCheck => {
            let tid = current_tid();
            if mutex.owner.load(Ordering::Relaxed) == tid {
                return libc::EDEADLK;
            }
            mutex.acquire_owned(tid);
        }
    }
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_mutex_trylock(mutex: &PthreadMutex) -> c_int {
    signature_matches_libc!(libc::pthread_mutex_trylock(std::mem::transmute(mutex)));
    if mutex.kind() == MutexKind::Recursive {
        let tid = current_tid();
        if mutex.owner.load(Ordering::Relaxed) == tid {
            *mutex.recursion.get() += 1;
            return 0;
        }
    }
    if !mutex.try_acquire() {
        return libc::EBUSY;
    }
    if mutex.kind() != MutexKind::Normal {
        mutex.owner.store(current_tid(), Ordering::Relaxed);
        *mutex.recursion.get() = 1;
    }
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_mutex_unlock(mutex: &PthreadMutex) -> c_int {
    signature_matches_libc!(libc::pthread_mutex_unlock(std::mem::transmute(mutex)));
    if mutex.kind() != MutexKind::Normal {
        if mutex.owner.load(Ordering::Relaxed) != current_tid() {
            return libc::EPERM;
        }
        let recursion = mutex.recursion.get();
        if *recursion > 1 {
            *recursion -= 1;
            return 0;
        }
        *recursion = 0;
        mutex.owner.store(0, Ordering::Relaxed);
    }
    mutex.release();
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_mutex_init(
    mutex: &mut PthreadMutex,
    attr: Option<NonNull<PthreadMutexAttr>>,
) -> c_int {
    signature_matches_libc!(libc::pthread_mutex_init(
        std::mem::transmute(mutex),
        std::mem::transmute(attr)
    ));
    let kind = attr
        .map(|attr| attr.as_ref().kind)
        .unwrap_or(MutexKind::Normal as i32);
    *mutex = PthreadMutex::new(kind);
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_mutex_destroy(mutex: &PthreadMutex) -> c_int {
    signature_matches_libc!(libc::pthread_mutex_destroy(std::mem::transmute(mutex)));
    0
}

/// glibc's `pthread_mutexattr_t` is a 4-byte blob; we keep only the kind in it.
#[repr(C)]
struct PthreadMutexAttr {
    kind: i32,
}

const _: () = assert!(size_of::<PthreadMutexAttr>() == size_of::<libc::pthread_mutexattr_t>());

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_mutexattr_init(attr: &mut PthreadMutexAttr) -> c_int {
    signature_matches_libc!(libc::pthread_mutexattr_init(std::mem::transmute(attr)));
    *attr = PthreadMutexAttr {
        kind: MutexKind::Normal as i32,
    };
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_mutexattr_destroy(attr: &PthreadMutexAttr) -> c_int {
    signature_matches_libc!(libc::pthread_mutexattr_destroy(std::mem::transmute(attr)));
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_mutexattr_settype(attr: &mut PthreadMutexAttr, kind: c_int) -> c_int {
    signature_matches_libc!(libc::pthread_mutexattr_settype(
        std::mem::transmute(attr),
        kind
    ));
    if MutexKind::from_repr(kind).is_none() {
        return libc::EINVAL;
    }
    attr.kind = kind;
    0
}
