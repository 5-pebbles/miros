use core::{
    ffi::c_int,
    ptr::{self, NonNull},
};
use std::{
    cell::UnsafeCell,
    mem::offset_of,
    sync::atomic::{AtomicU32, Ordering},
};

use arbitrary_int::u2;
use bitbybit::{bitenum, bitfield};

use crate::{
    libc::{
        errno::Errno,
        threads::{current_tid, futex_wait, futex_wake},
    },
    signature_matches_libc,
};

const UNLOCKED: u32 = 0;
const LOCKED: u32 = 1;
const CONTENDED: u32 = 2;

/// glibc's default `glibc.pthread.spin_count` tunable value.
const DEFAULT_ADAPTIVE_SPINS: u16 = 100;

#[derive(PartialEq)]
#[bitenum(u2, exhaustive = true)]
enum MutexKind {
    Normal = 0,
    Recursive = 1,
    ErrorCheck = 2,
    Adaptive = 3,
}

// glibc's `__kind`: the pure type sits in the low 2 bits; bits 4/5/6 mark robust/priority-inherit/priority-protect.
#[bitfield(u32)]
struct MutexKindField {
    #[bits(0..=1, rw)]
    kind_type: MutexKind,
    #[bit(4, rw)]
    robust: bool,
    #[bit(5, rw)]
    prio_inherit: bool,
    #[bit(6, rw)]
    prio_protect: bool,
}

/// glibc's `__pthread_list_t`: robust-mutex chain links.
/// The kernel walks these pointers through the per-thread robust list on owner death.
#[repr(C)]
struct RobustList {
    prev: *mut RobustList,
    next: *mut RobustList,
}

/// glibc's `PTHREAD_PROCESS_SHARED` blob in shared memory can be locked by another program who isn't linked by miros.
/// Soo, we require identical structure to glibc...
#[repr(C, align(8))]
pub struct PthreadMutex {
    state: AtomicU32,
    recursion: UnsafeCell<u32>,
    owner: AtomicU32,
    _number_of_users: u32,
    kind: MutexKindField,
    spins: u16,
    // TSX lock elision is unsupported (for now?); glibc leaves this zero when elision is off.
    // TODO: Should we add elision support? Probably?
    _elision: u16,
    // TODO: Implment the robust list.
    _robust_list: RobustList,
}

const _: () = assert!(size_of::<PthreadMutex>() == size_of::<libc::pthread_mutex_t>());
const _: () = assert!(align_of::<PthreadMutex>() == align_of::<libc::pthread_mutex_t>());
// glibc x86_64 `__pthread_mutex_s`: __lock @0, __count @4, __owner @8, __nusers @12, __kind @16, __spins @20, __elision @22, __list @24. The static initializers write only `kind`.
const _: () = {
    assert!(offset_of!(PthreadMutex, state) == 0);
    assert!(offset_of!(PthreadMutex, recursion) == 4);
    assert!(offset_of!(PthreadMutex, owner) == 8);
    assert!(offset_of!(PthreadMutex, _number_of_users) == 12);
    assert!(offset_of!(PthreadMutex, kind) == 16);
    assert!(offset_of!(PthreadMutex, spins) == 20);
    assert!(offset_of!(PthreadMutex, _elision) == 22);
    assert!(offset_of!(PthreadMutex, _robust_list) == 24);
};

// SAFETY: `recursion` is only touched by the thread whose tid is in `owner`.
unsafe impl Sync for PthreadMutex {}

impl PthreadMutex {
    const fn new(kind: MutexKindField, spins: u16) -> Self {
        Self {
            state: AtomicU32::new(UNLOCKED),
            recursion: UnsafeCell::new(0),
            owner: AtomicU32::new(0),
            _number_of_users: 0,
            kind,
            spins,
            _elision: 0,
            _robust_list: RobustList {
                prev: ptr::null_mut(),
                next: ptr::null_mut(),
            },
        }
    }

    fn kind(&self) -> MutexKind {
        self.kind.kind_type()
    }

    /// Robust, priority-inherit, or priority-protect flag set in `kind`; miros supports none of them.
    fn has_unsupported_flags(&self) -> bool {
        self.kind.robust() || self.kind.prio_inherit() || self.kind.prio_protect()
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

    /// Fully release for `pthread_cond_wait`, returning the recursion count the caller must hand to `acquire_after_wait`.
    pub unsafe fn release_for_wait(&self) -> u32 {
        if self.kind() != MutexKind::Normal {
            self.owner.store(0, Ordering::Relaxed);
            let recursion = *self.recursion.get();
            *self.recursion.get() = 0;
            self.release();
            recursion
        } else {
            self.release();
            0
        }
    }

    /// Re-acquire after `pthread_cond_wait` returns, restoring the recursion count from `release_for_wait`.
    pub unsafe fn acquire_after_wait(&self, recursion: u32) {
        match self.kind() {
            MutexKind::Normal => self.acquire(),
            _ => {
                self.acquire_owned(current_tid());
                *self.recursion.get() = recursion;
            }
        }
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_mutex_lock(mutex: &PthreadMutex) -> c_int {
    signature_matches_libc!(libc::pthread_mutex_lock(std::mem::transmute(mutex)));
    if mutex.has_unsupported_flags() {
        return Errno::NOTSUP.into();
    }
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
                return Errno::DEADLK.into();
            }
            mutex.acquire_owned(tid);
        }
        MutexKind::Adaptive => {
            // Statically-initialized adaptive mutexes have `spins == 0`; fall back to the default.
            let spins = if mutex.spins == 0 {
                DEFAULT_ADAPTIVE_SPINS
            } else {
                mutex.spins
            };
            for _ in 0..spins {
                if mutex.try_acquire() {
                    return 0;
                }
                core::hint::spin_loop();
            }
            mutex.acquire();
        }
    }
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_mutex_trylock(mutex: &PthreadMutex) -> c_int {
    signature_matches_libc!(libc::pthread_mutex_trylock(std::mem::transmute(mutex)));
    if mutex.has_unsupported_flags() {
        return Errno::NOTSUP.into();
    }
    if mutex.kind() == MutexKind::Recursive {
        let tid = current_tid();
        if mutex.owner.load(Ordering::Relaxed) == tid {
            *mutex.recursion.get() += 1;
            return 0;
        }
    }
    if !mutex.try_acquire() {
        return Errno::BUSY.into();
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
    if mutex.has_unsupported_flags() {
        return Errno::NOTSUP.into();
    }
    if mutex.kind() != MutexKind::Normal {
        if mutex.owner.load(Ordering::Relaxed) != current_tid() {
            return Errno::PERM.into();
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
        .map(|attr| MutexKindField::new_with_raw_value(attr.as_ref().raw_value()))
        .unwrap_or(MutexKindField::ZERO);
    let spins = if kind.kind_type() == MutexKind::Adaptive {
        DEFAULT_ADAPTIVE_SPINS
    } else {
        0
    };
    *mutex = PthreadMutex::new(kind, spins);
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_mutex_destroy(mutex: &PthreadMutex) -> c_int {
    signature_matches_libc!(libc::pthread_mutex_destroy(std::mem::transmute(mutex)));
    0
}

/// glibc's `pthread_mutexattr_t` is a 4-byte blob; we keep only the kind in it.
#[bitfield(u32)]
struct PthreadMutexAttr {
    #[bits(0..=1, rw)]
    kind: MutexKind,
}

const _: () = assert!(size_of::<PthreadMutexAttr>() == size_of::<libc::pthread_mutexattr_t>());

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_mutexattr_init(attr: &mut PthreadMutexAttr) -> c_int {
    signature_matches_libc!(libc::pthread_mutexattr_init(std::mem::transmute(attr)));
    *attr = PthreadMutexAttr::ZERO.with_kind(MutexKind::Normal);
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
    let Some(valid_kind) = u8::try_from(kind)
        .ok()
        .and_then(|value| u2::try_new(value).ok())
        .map(MutexKind::new_with_raw_value)
    else {
        return Errno::INVAL.into();
    };
    *attr = attr.with_kind(valid_kind);
    0
}
