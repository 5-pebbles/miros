use core::{ffi::c_int, mem::offset_of, ptr::NonNull};
use std::sync::atomic::Ordering;

use arbitrary_int::{u29, u30, u63};
use atomic::Atomic;
use bitbybit::{bitenum, bitfield};
use bytemuck::NoUninit;

use crate::{
    libc::{
        errno::Errno,
        threads::{futex_wait, futex_wake, mutex::PthreadMutex},
    },
    signature_matches_libc,
};

/// Drepper three-state lock signalers hold while delivering signals.
#[bitenum(u2, exhaustive = true)]
#[derive(PartialEq, Eq)]
enum SignalerLock {
    Unlocked = 0,
    Locked = 1,
    LockedWithWaiters = 2,
    Reserved = 3,
}

#[bitfield(u32)]
#[derive(NoUninit)]
struct LockedOriginalSize {
    #[bits(0..=1, rw)]
    signaler_lock: SignalerLock,
    #[bits(2..=31, rw)]
    original_size: u30,
}

#[bitfield(u64)]
#[derive(NoUninit)]
struct WaitSequence {
    #[bit(0, rw)]
    group_two_slot: bool,
    #[bits(1..=63, rw)]
    ticket: u63,
}

#[bitfield(u32)]
#[derive(NoUninit)]
struct WaiterReferences {
    #[bit(0, rw)]
    pshared: bool,
    #[bit(1, rw)]
    monotonic_clock: bool,
    #[bit(2, rw)]
    destroy_wake_request: bool,
    #[bits(3..=31, rw)]
    waiters: u29,
}

/// `fetch_update` with an infallible mapping, returning the previous value.
fn fetch_map<T: NoUninit>(
    word: &Atomic<T>,
    set: Ordering,
    fetch: Ordering,
    map: impl FnMut(T) -> T,
) -> T {
    let mut map = map;
    let Ok(previous) = word.fetch_update(set, fetch, |value| Some(map(value))) else {
        unreachable!()
    };
    previous
}

/// glibc's `__pthread_cond_s`, protocol-compatible so PTHREAD_PROCESS_SHARED condvars interoperate with real glibc processes.
/// Cancellation and timed waits are unimplemented, so waiters here never cancel and `group_sizes` stays accurate.
#[repr(C, align(8))]
struct PthreadCond {
    wait_sequence: Atomic<WaitSequence>,
    /// Ticket position where group one starts (inclusive); advancing it closes the group.
    group_one_start: Atomic<u64>,
    /// Unsignaled waiters per group slot. Group two's entry stays zero until the group is formed.
    group_sizes: [Atomic<u32>; 2],
    group_one_original_size: Atomic<LockedOriginalSize>,
    waiter_references: Atomic<WaiterReferences>,
    /// Per-slot futex words holding `group_one_start` plus the signals delivered to that group.
    group_signals: [Atomic<u32>; 2],
    _reserved_1: u32,
    _reserved_2: u32,
}

const _: () = assert!(size_of::<PthreadCond>() == size_of::<libc::pthread_cond_t>());
const _: () = assert!(align_of::<PthreadCond>() == align_of::<libc::pthread_cond_t>());
// glibc x86_64 `__pthread_cond_s`: __wseq @0, __g1_start @8, __g_size @16, __g1_orig_size @24,
// __wrefs @28, __g_signals @32, __unused_initialized_1 @40, __unused_initialized_2 @44.
const _: () = {
    assert!(offset_of!(PthreadCond, wait_sequence) == 0);
    assert!(offset_of!(PthreadCond, group_one_start) == 8);
    assert!(offset_of!(PthreadCond, group_sizes) == 16);
    assert!(offset_of!(PthreadCond, group_one_original_size) == 24);
    assert!(offset_of!(PthreadCond, waiter_references) == 28);
    assert!(offset_of!(PthreadCond, group_signals) == 32);
    assert!(offset_of!(PthreadCond, _reserved_1) == 40);
    assert!(offset_of!(PthreadCond, _reserved_2) == 44);
};

impl PthreadCond {
    const fn new() -> Self {
        Self {
            wait_sequence: Atomic::new(WaitSequence::ZERO),
            group_one_start: Atomic::new(0),
            group_sizes: [Atomic::new(0), Atomic::new(0)],
            group_one_original_size: Atomic::new(LockedOriginalSize::ZERO),
            waiter_references: Atomic::new(WaiterReferences::ZERO),
            group_signals: [Atomic::new(0), Atomic::new(0)],
            _reserved_1: 0,
            _reserved_2: 0,
        }
    }

    fn signals(&self, group: usize) -> &Atomic<u32> {
        self.group_signals.get(group).unwrap()
    }

    fn size(&self, group: usize) -> &Atomic<u32> {
        self.group_sizes.get(group).unwrap()
    }

    fn original_size(&self) -> u30 {
        self.group_one_original_size
            .load(Ordering::Relaxed)
            .original_size()
    }

    /// Only called with the lock held; a concurrent contender may still flip the lock bits, so rewrite them if the swap observed a transition.
    fn set_original_size(&self, size: u30) {
        let mut state = Err(self.group_one_original_size.load(Ordering::Relaxed));
        while let Err(previous) = state {
            if previous.original_size() == size {
                return;
            }
            state = self.group_one_original_size.compare_exchange(
                previous,
                previous.with_original_size(size),
                Ordering::Acquire,
                Ordering::Relaxed,
            );
        }
    }

    fn acquire_lock(&self) {
        let mut state = self.group_one_original_size.load(Ordering::Relaxed);
        while state.signaler_lock() == SignalerLock::Unlocked {
            match self.group_one_original_size.compare_exchange_weak(
                state,
                state.with_signaler_lock(SignalerLock::Locked),
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(actual) => state = actual,
            }
        }
        loop {
            while state.signaler_lock() != SignalerLock::LockedWithWaiters {
                match self.group_one_original_size.compare_exchange_weak(
                    state,
                    state.with_signaler_lock(SignalerLock::LockedWithWaiters),
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        if state.signaler_lock() == SignalerLock::Unlocked {
                            return;
                        }
                        break;
                    }
                    Err(actual) => state = actual,
                }
            }
            let expected = state.with_signaler_lock(SignalerLock::LockedWithWaiters);
            futex_wait(&self.group_one_original_size, expected.raw_value());
            state = self.group_one_original_size.load(Ordering::Relaxed);
        }
    }

    fn release_lock(&self) {
        let previous = fetch_map(
            &self.group_one_original_size,
            Ordering::Release,
            Ordering::Relaxed,
            |state| state.with_signaler_lock(SignalerLock::Unlocked),
        );
        if previous.signaler_lock() == SignalerLock::LockedWithWaiters {
            futex_wake(&self.group_one_original_size, 1);
        }
    }

    /// Close group one and promote group two to it. Returns false when group two has no waiters to inherit.
    fn switch_group_one(&self, observed_sequence: u64, group_one: &mut usize) -> bool {
        let old_group_one = *group_one;
        let old_original_size = self.original_size();
        let old_start = self.group_one_start.load(Ordering::Relaxed);
        let new_start = old_start + u64::from(old_original_size);
        let unaccounted = observed_sequence.wrapping_sub(new_start) as u32;
        if unaccounted.wrapping_add(self.size(old_group_one ^ 1).load(Ordering::Relaxed)) == 0 {
            return false;
        }

        self.group_one_start
            .fetch_add(u64::from(old_original_size), Ordering::Relaxed);
        // Flipping the slot publishes the switch and redirects future waiters to the other slot in one atomic.
        let switched_sequence = fetch_map(
            &self.wait_sequence,
            Ordering::Release,
            Ordering::Relaxed,
            |sequence| sequence.with_group_two_slot(!sequence.group_two_slot()),
        )
        .ticket()
        .value();
        let new_group_one = old_group_one ^ 1;
        *group_one = new_group_one;

        self.signals(new_group_one)
            .store(new_start as u32, Ordering::Release);
        let original_size = switched_sequence.wrapping_sub(new_start) as u32;
        self.set_original_size(u30::new(original_size));
        self.size(new_group_one)
            .fetch_add(original_size, Ordering::Relaxed);
        self.size(new_group_one).load(Ordering::Relaxed) != 0
    }

    fn confirm_wakeup(&self) {
        let previous = fetch_map(
            &self.waiter_references,
            Ordering::Release,
            Ordering::Relaxed,
            |references| references.with_waiters(references.waiters() - u29::new(1)),
        );
        if previous.destroy_wake_request() && previous.waiters() == u29::new(1) {
            futex_wake(&self.waiter_references, i32::MAX);
        }
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_cond_wait(cond: &PthreadCond, mutex: &PthreadMutex) -> c_int {
    signature_matches_libc!(libc::pthread_cond_wait(
        std::mem::transmute(cond),
        std::mem::transmute(mutex)
    ));

    // Registration must complete before the mutex release: a signaler ordered after the release must see this waiter.
    let ticket = fetch_map(
        &cond.wait_sequence,
        Ordering::Acquire,
        Ordering::Acquire,
        |sequence| sequence.with_ticket(sequence.ticket() + u63::new(1)),
    );
    let group = usize::from(ticket.group_two_slot());
    let position = ticket.ticket().value();
    fetch_map(
        &cond.waiter_references,
        Ordering::Relaxed,
        Ordering::Relaxed,
        |references| references.with_waiters(references.waiters() + u29::new(1)),
    );

    let recursion = mutex.release_for_wait();

    let signals = cond.signals(group);
    loop {
        let available = signals.load(Ordering::Acquire);
        let group_one_start = cond.group_one_start.load(Ordering::Relaxed);
        if position < group_one_start {
            // Our group was closed with a signal provided for every member.
            break;
        }
        if available.wrapping_sub(group_one_start as u32) as i32 > 0 {
            if signals
                .compare_exchange_weak(
                    available,
                    available.wrapping_sub(1),
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
            continue;
        }
        futex_wait(signals, available);
    }

    cond.confirm_wakeup();
    mutex.acquire_after_wait(recursion);
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_cond_signal(cond: &PthreadCond) -> c_int {
    signature_matches_libc!(libc::pthread_cond_signal(std::mem::transmute(cond)));
    if cond.waiter_references.load(Ordering::Relaxed).waiters() == u29::new(0) {
        return 0;
    }
    cond.acquire_lock();

    let ticket_state = cond.wait_sequence.load(Ordering::Relaxed);
    let mut group_one = usize::from(!ticket_state.group_two_slot());
    let next_position = ticket_state.ticket().value();

    let mut wake = false;
    if cond.size(group_one).load(Ordering::Relaxed) != 0
        || cond.switch_group_one(next_position, &mut group_one)
    {
        cond.signals(group_one).fetch_add(1, Ordering::Relaxed);
        cond.size(group_one).fetch_sub(1, Ordering::Relaxed);
        wake = true;
    }

    cond.release_lock();
    if wake {
        futex_wake(cond.signals(group_one), 1);
    }
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_cond_broadcast(cond: &PthreadCond) -> c_int {
    signature_matches_libc!(libc::pthread_cond_broadcast(std::mem::transmute(cond)));
    if cond.waiter_references.load(Ordering::Relaxed).waiters() == u29::new(0) {
        return 0;
    }
    cond.acquire_lock();

    let ticket_state = cond.wait_sequence.load(Ordering::Relaxed);
    let group_two = usize::from(ticket_state.group_two_slot());
    let mut group_one = group_two ^ 1;
    let next_position = ticket_state.ticket().value();

    let remaining = cond.size(group_one).load(Ordering::Relaxed);
    if remaining != 0 {
        cond.signals(group_one)
            .fetch_add(remaining, Ordering::Relaxed);
        cond.size(group_one).store(0, Ordering::Relaxed);
        // Group one must be awake before the role switch below repurposes its slot.
        futex_wake(cond.signals(group_one), i32::MAX);
    }

    let mut wake = false;
    if cond.switch_group_one(next_position, &mut group_one) {
        let size = cond.size(group_one).load(Ordering::Relaxed);
        cond.signals(group_one).fetch_add(size, Ordering::Relaxed);
        cond.size(group_one).store(0, Ordering::Relaxed);
        wake = true;
    }

    cond.release_lock();
    if wake {
        futex_wake(cond.signals(group_one), i32::MAX);
    }
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_cond_init(
    cond: &mut PthreadCond,
    attr: Option<NonNull<libc::pthread_condattr_t>>,
) -> c_int {
    signature_matches_libc!(libc::pthread_cond_init(
        std::mem::transmute(cond),
        std::mem::transmute(attr)
    ));
    *cond = PthreadCond::new();
    if let Some(attr) = attr {
        let attr = unsafe { attr.cast::<PthreadCondAttr>().as_ref() };
        let references = WaiterReferences::ZERO
            .with_pshared(attr.pshared())
            .with_monotonic_clock(attr.monotonic_clock());
        cond.waiter_references.store(references, Ordering::Relaxed);
    }
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_cond_destroy(cond: &PthreadCond) -> c_int {
    signature_matches_libc!(libc::pthread_cond_destroy(std::mem::transmute(cond)));
    let mut references = fetch_map(
        &cond.waiter_references,
        Ordering::Acquire,
        Ordering::Acquire,
        |references| references.with_destroy_wake_request(true),
    );
    // The fetch returns the pre-flag value; the futex word already carries the flag.
    references = references.with_destroy_wake_request(true);
    while references.waiters() != u29::new(0) {
        futex_wait(&cond.waiter_references, references.raw_value());
        references = cond.waiter_references.load(Ordering::Acquire);
    }
    0
}

/// glibc's `struct pthread_condattr { int value; }`.
#[bitfield(u32)]
struct PthreadCondAttr {
    #[bit(0, rw)]
    pshared: bool,
    #[bit(1, rw)]
    monotonic_clock: bool,
}

const _: () = assert!(size_of::<PthreadCondAttr>() == size_of::<libc::pthread_condattr_t>());

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_condattr_init(attr: &mut PthreadCondAttr) -> c_int {
    signature_matches_libc!(libc::pthread_condattr_init(std::mem::transmute(attr)));
    *attr = PthreadCondAttr::new_with_raw_value(0);
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_condattr_destroy(attr: &PthreadCondAttr) -> c_int {
    signature_matches_libc!(libc::pthread_condattr_destroy(std::mem::transmute(attr)));
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_condattr_setpshared(
    attr: &mut PthreadCondAttr,
    pshared: c_int,
) -> c_int {
    signature_matches_libc!(libc::pthread_condattr_setpshared(
        std::mem::transmute(attr),
        pshared
    ));
    match pshared {
        libc::PTHREAD_PROCESS_PRIVATE => *attr = attr.with_pshared(false),
        libc::PTHREAD_PROCESS_SHARED => *attr = attr.with_pshared(true),
        _ => return Errno::INVAL.into(),
    }
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_condattr_getpshared(
    attr: &PthreadCondAttr,
    pshared: &mut c_int,
) -> c_int {
    signature_matches_libc!(libc::pthread_condattr_getpshared(
        std::mem::transmute(attr),
        std::mem::transmute(pshared)
    ));
    *pshared = if attr.pshared() {
        libc::PTHREAD_PROCESS_SHARED
    } else {
        libc::PTHREAD_PROCESS_PRIVATE
    };
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_condattr_setclock(
    attr: &mut PthreadCondAttr,
    clock_id: libc::clockid_t,
) -> c_int {
    signature_matches_libc!(libc::pthread_condattr_setclock(
        std::mem::transmute(attr),
        clock_id
    ));
    match clock_id {
        libc::CLOCK_REALTIME => *attr = attr.with_monotonic_clock(false),
        libc::CLOCK_MONOTONIC => *attr = attr.with_monotonic_clock(true),
        _ => return Errno::INVAL.into(),
    }
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_condattr_getclock(
    attr: &PthreadCondAttr,
    clock_id: &mut libc::clockid_t,
) -> c_int {
    signature_matches_libc!(libc::pthread_condattr_getclock(
        std::mem::transmute(attr),
        std::mem::transmute(clock_id)
    ));
    *clock_id = if attr.monotonic_clock() {
        libc::CLOCK_MONOTONIC
    } else {
        libc::CLOCK_REALTIME
    };
    0
}
