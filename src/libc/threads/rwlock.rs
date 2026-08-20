use std::{
    ffi::c_int,
    mem,
    mem::offset_of,
    ops::Deref,
    ptr::NonNull,
    sync::atomic::{
        fence, AtomicU32,
        Ordering::{self, Acquire, Relaxed, Release},
    },
};

use arbitrary_int::{u29, u31};
use atomic::Atomic;
use bitbybit::bitfield;
use bytemuck::NoUninit;

use crate::{
    libc::{
        errno::Errno,
        threads::{current_tid, futex_wait, futex_wake, FetchMap},
    },
    signature_matches_libc,
};

#[derive(Clone, Copy)]
#[repr(i32)]
enum PthreadLockKind {
    // Hardcoded because glibc exposes the rwlock preferences as an anonymous enum, so the libc crate omits them.
    PreferReader = 0,
    PreferWriter = 1,
    PreferWriterNonrecursive = 2,
}

impl TryFrom<c_int> for PthreadLockKind {
    type Error = Errno;

    fn try_from(preference: c_int) -> Result<Self, Self::Error> {
        match preference {
            0 => Ok(Self::PreferReader),
            1 => Ok(Self::PreferWriter),
            2 => Ok(Self::PreferWriterNonrecursive),
            _ => Err(Errno::INVAL),
        }
    }
}

#[derive(Clone, Copy)]
#[repr(i32)]
enum PthreadProcessShared {
    Private = libc::PTHREAD_PROCESS_PRIVATE,
    Shared = libc::PTHREAD_PROCESS_SHARED,
}

impl TryFrom<c_int> for PthreadProcessShared {
    type Error = Errno;

    fn try_from(pshared: c_int) -> Result<Self, Self::Error> {
        match pshared {
            libc::PTHREAD_PROCESS_PRIVATE => Ok(Self::Private),
            libc::PTHREAD_PROCESS_SHARED => Ok(Self::Shared),
            _ => Err(Errno::INVAL),
        }
    }
}

#[bitfield(u32)]
#[derive(NoUninit)]
struct Readers {
    #[bit(0, rw)]
    write_phase: bool,
    #[bit(1, rw)]
    writer_locked: bool,
    #[bit(2, rw)]
    readers_waiting: bool,
    #[bits(3..=31, rw)]
    readers: u29,
}

impl Readers {
    fn has_readers(self) -> bool {
        self.readers() != u29::new(0)
    }

    fn incremented(self) -> Self {
        self.with_readers(self.readers() + u29::new(1))
    }

    fn decremented(self) -> Self {
        self.with_readers(self.readers() - u29::new(1))
    }
}

#[repr(transparent)]
struct AtomicReaders(Atomic<Readers>);

impl AtomicReaders {
    const READER_INCREMENT: u32 = 1 << 3;
    const MAX_READERS: u29 = u29::new(1 << 28);

    const fn new(readers: Readers) -> Self {
        Self(Atomic::new(readers))
    }

    fn as_raw(&self) -> &AtomicU32 {
        // SAFETY: transparent newtype over Atomic<Readers>, itself transparent over a NoUninit u32 bitfield.
        unsafe { mem::transmute::<&Self, &AtomicU32>(self) }
    }

    fn fetch_add(&self, count: u32, order: Ordering) -> Readers {
        let increment = count * Self::READER_INCREMENT;
        Readers::new_with_raw_value(self.as_raw().fetch_add(increment, order) + increment)
    }
}

impl Deref for AtomicReaders {
    type Target = Atomic<Readers>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// `handover` is the token a primary writer leaves for a successor.
#[bitfield(u32)]
#[derive(NoUninit)]
struct Writers {
    #[bits(0..=30, rw)]
    waiting: u31,
    #[bit(31, rw)]
    handover: bool,
}

#[repr(transparent)]
struct AtomicWriters(Atomic<Writers>);

impl AtomicWriters {
    const fn new(writers: Writers) -> Self {
        Self(Atomic::new(writers))
    }

    fn as_raw(&self) -> &AtomicU32 {
        // SAFETY: transparent newtype over Atomic<Writers>, itself transparent over a NoUninit u32 bitfield.
        unsafe { mem::transmute::<&Self, &AtomicU32>(self) }
    }

    fn fetch_add(&self, count: u32, order: Ordering) -> Writers {
        Writers::new_with_raw_value(self.as_raw().fetch_add(count, order) + count)
    }

    fn fetch_sub(&self, count: u32, order: Ordering) -> Writers {
        Writers::new_with_raw_value(self.as_raw().fetch_sub(count, order) - count)
    }
}

impl Deref for AtomicWriters {
    type Target = Atomic<Writers>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// `phase` shadows the write-phase bit; `used` marks potential waiters so wakers can skip the syscall.
#[bitfield(u32)]
#[derive(NoUninit)]
struct PhaseWord {
    #[bit(0, rw)]
    phase: bool,
    #[bit(1, rw)]
    used: bool,
}

#[repr(transparent)]
struct AtomicPhaseWord(Atomic<PhaseWord>);

impl AtomicPhaseWord {
    const fn new(word: PhaseWord) -> Self {
        Self(Atomic::new(word))
    }
}

impl Deref for AtomicPhaseWord {
    type Target = Atomic<PhaseWord>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn tid() -> u32 {
    // SAFETY: pthread entry points only run on threads with an installed thread pointer.
    unsafe { current_tid() }
}

// TODO: Timed waits.
/// glibc's `__pthread_rwlock_arch_t`, protocol-compatible so PTHREAD_PROCESS_SHARED rwlocks interoperate with real glibc processes.
#[repr(C, align(8))]
struct PthreadRwlock {
    readers: AtomicReaders,
    writers: AtomicWriters,
    write_phase_futex: AtomicPhaseWord,
    writers_futex: AtomicPhaseWord,
    _pad3: u32,
    _pad4: u32,
    current_writer: AtomicU32,
    shared: c_int,
    _pad1: u64,
    _pad2: u64,
    flags: c_int,
}

const _: () = assert!(size_of::<PthreadRwlock>() == size_of::<libc::pthread_rwlock_t>());
const _: () = assert!(align_of::<PthreadRwlock>() == align_of::<libc::pthread_rwlock_t>());
// glibc x86_64 `__pthread_rwlock_arch_t`: __readers @0, __writers @4, __wrphase_futex @8, __writers_futex @12,
// __pad3 @16, __pad4 @20, __cur_writer @24, __shared @28, __pad1 @32, __pad2 @40, __flags @48.
const _: () = {
    assert!(offset_of!(PthreadRwlock, readers) == 0);
    assert!(offset_of!(PthreadRwlock, writers) == 4);
    assert!(offset_of!(PthreadRwlock, write_phase_futex) == 8);
    assert!(offset_of!(PthreadRwlock, writers_futex) == 12);
    assert!(offset_of!(PthreadRwlock, _pad3) == 16);
    assert!(offset_of!(PthreadRwlock, _pad4) == 20);
    assert!(offset_of!(PthreadRwlock, current_writer) == 24);
    assert!(offset_of!(PthreadRwlock, shared) == 28);
    assert!(offset_of!(PthreadRwlock, _pad1) == 32);
    assert!(offset_of!(PthreadRwlock, _pad2) == 40);
    assert!(offset_of!(PthreadRwlock, flags) == 48);
};

impl PthreadRwlock {
    const fn new(kind: PthreadLockKind, pshared: PthreadProcessShared) -> Self {
        Self {
            readers: AtomicReaders::new(Readers::ZERO),
            writers: AtomicWriters::new(Writers::ZERO),
            write_phase_futex: AtomicPhaseWord::new(PhaseWord::ZERO),
            writers_futex: AtomicPhaseWord::new(PhaseWord::ZERO),
            _pad3: 0,
            _pad4: 0,
            current_writer: AtomicU32::new(0),
            shared: pshared as c_int,
            _pad1: 0,
            _pad2: 0,
            flags: kind as c_int,
        }
    }

    fn prefer_writer(&self) -> bool {
        self.flags != PthreadLockKind::PreferReader as c_int
    }

    fn prefer_writer_nonrecursive(&self) -> bool {
        self.flags == PthreadLockKind::PreferWriterNonrecursive as c_int
    }

    /// Park until the write-phase futex's phase bit reads `target`: the explicit hand-over from the phase owner.
    fn await_phase_handover(&self, target: bool) {
        loop {
            let word = self.write_phase_futex.load(Relaxed);
            if word.phase() == target {
                return;
            }
            let expected = word.with_used(true);
            if !word.used()
                && self
                    .write_phase_futex
                    .compare_exchange_weak(word, expected, Relaxed, Relaxed)
                    .is_err()
            {
                continue;
            }
            futex_wait(&self.write_phase_futex, expected.raw_value());
        }
    }

    fn read_lock(&self) -> Result<(), Errno> {
        if self.current_writer.load(Relaxed) == tid() {
            return Err(Errno::DEADLK);
        }
        // Writer-preferred with no recursive readers: wait for the primary writer without extending the read phase.
        if self.prefer_writer_nonrecursive() {
            let mut state = self.readers.load(Relaxed);
            while !state.write_phase() && state.writer_locked() && state.has_readers() {
                match self.readers.compare_exchange_weak(
                    state,
                    state.with_readers_waiting(true),
                    Relaxed,
                    Relaxed,
                ) {
                    Ok(_) => {
                        // ABA is harmless: the flag only tracks the state of `readers`, and every waiter sets it under the same conditions.
                        while {
                            state = self.readers.load(Relaxed);
                            state.readers_waiting()
                        } {
                            futex_wait(&self.readers, state.raw_value());
                        }
                    }
                    Err(actual) => state = actual,
                }
            }
        }
        // Acquire so we synchronize with prior writers and the previous phase's last reader.
        let state = self.readers.fetch_add(1, Acquire);
        // With fewer than 2^28 threads a count past MAX_READERS is a true overflow; the undo is a CAS, not a fetch_sub, because a concurrent release could otherwise make us the last reader out and skip the hand-over.
        if state.readers() >= AtomicReaders::MAX_READERS {
            self.readers
                .fetch_map(Relaxed, Relaxed, Readers::decremented);
            return Err(Errno::AGAIN);
        }
        if !state.write_phase() {
            return Ok(());
        }
        // Write phase with no primary writer: any reader may start the read phase.
        if self
            .readers
            .fetch_update(Acquire, Relaxed, |state| {
                (state.write_phase() && !state.writer_locked())
                    .then(|| state.with_write_phase(false))
            })
            .is_ok()
        {
            // We started the read phase, so we owe the writer's hand-over steps: parked readers cannot distinguish us from one.
            if self.write_phase_futex.swap(PhaseWord::ZERO, Relaxed).used() {
                futex_wake(&self.write_phase_futex, i32::MAX);
            }
            return Ok(());
        }
        // A writer holds or is taking the lock: wait for explicit hand-over, then confirm through `readers` because the futex word may be stale.
        loop {
            self.await_phase_handover(false);
            if !self.readers.load(Acquire).write_phase() {
                self.await_phase_handover(false);
                return Ok(());
            }
        }
    }

    fn try_read_lock(&self) -> Result<(), Errno> {
        let mut state = self.readers.load(Relaxed);
        let previous = loop {
            if state.readers() >= AtomicReaders::MAX_READERS {
                return Err(Errno::AGAIN);
            }
            let next = if !state.write_phase() {
                if state.writer_locked() && self.prefer_writer_nonrecursive() {
                    return Err(Errno::BUSY);
                }
                state.incremented()
            } else {
                if state.writer_locked() {
                    return Err(Errno::BUSY);
                }
                // Idle write phase: acquire and start the read phase ourselves.
                state.incremented().with_write_phase(false)
            };
            match self
                .readers
                .compare_exchange_weak(state, next, Acquire, Relaxed)
            {
                Ok(_) => break state,
                Err(actual) => state = actual,
            }
        };
        if previous.write_phase() {
            // Same hand-over duty as in read_lock.
            if self.write_phase_futex.swap(PhaseWord::ZERO, Relaxed).used() {
                futex_wake(&self.write_phase_futex, i32::MAX);
            }
        }
        Ok(())
    }

    fn write_lock(&self) -> Result<(), Errno> {
        if self.current_writer.load(Relaxed) == tid() {
            return Err(Errno::DEADLK);
        }
        let prefer_writer = self.prefer_writer();
        // True once we parked on the writers futex: the used marker may be shared with other writers, so our store below must keep it.
        let mut may_share_used = false;
        let (mut state, _) = self
            .readers
            .fetch_map(Acquire, Acquire, |state| state.with_writer_locked(true));
        if state.writer_locked() {
            // Another primary writer exists: wait it out, preferring the writer-writer hand-over token.
            if prefer_writer {
                // The waiting count is bounded by the thread count, so bit 31 is unreachable.
                self.writers.fetch_add(1, Relaxed);
            }
            loop {
                if !state.writer_locked() {
                    match self.readers.compare_exchange_weak(
                        state,
                        state.with_writer_locked(true),
                        Acquire,
                        Relaxed,
                    ) {
                        Ok(_) => {
                            if prefer_writer {
                                self.writers.fetch_sub(1, Relaxed);
                            }
                            break;
                        }
                        Err(actual) => {
                            state = actual;
                            continue;
                        }
                    }
                }
                if prefer_writer {
                    let writers = self.writers.load(Relaxed);
                    if writers.handover() {
                        // Acquire so we inherit the handing-over writer's view of `readers`; an ABA on the token could otherwise resurrect a stale phase.
                        if self
                            .writers
                            .compare_exchange_weak(
                                writers,
                                writers
                                    .with_handover(false)
                                    .with_waiting(writers.waiting() - u31::new(1)),
                                Acquire,
                                Relaxed,
                            )
                            .is_ok()
                        {
                            state = self.readers.load(Relaxed);
                            break;
                        }
                        continue;
                    }
                }
                // Park only once the futex signals a primary writer (phase bit); otherwise reload `readers` and retry the primary-writer race.
                let word = self.writers_futex.load(Relaxed);
                if !word.phase() {
                    state = self.readers.load(Relaxed);
                    continue;
                }
                if !word.used()
                    && self
                        .writers_futex
                        .compare_exchange_weak(word, word.with_used(true), Relaxed, Relaxed)
                        .is_err()
                {
                    state = self.readers.load(Relaxed);
                    continue;
                }
                may_share_used = true;
                futex_wait(&self.writers_futex, word.with_used(true).raw_value());
                state = self.readers.load(Relaxed);
            }
            state = state.with_writer_locked(true);
        }
        self.writers_futex.store(
            PhaseWord::ZERO.with_phase(true).with_used(may_share_used),
            Relaxed,
        );
        // Become the owning writer once a write phase runs, starting it ourselves when the lock is idle.
        loop {
            if state.write_phase() {
                break;
            }
            while !state.has_readers() {
                match self.readers.compare_exchange_weak(
                    state,
                    state.with_write_phase(true),
                    Acquire,
                    Relaxed,
                ) {
                    Ok(_) => {
                        self.write_phase_futex
                            .store(PhaseWord::ZERO.with_phase(true), Relaxed);
                        break;
                    }
                    Err(actual) => state = actual,
                }
            }
            if state.write_phase() {
                break;
            }
            // Readers were active when we became primary writer: wait for explicit hand-over from the last one out.
            loop {
                self.await_phase_handover(true);
                if self.readers.load(Acquire).write_phase() {
                    self.await_phase_handover(true);
                    break;
                }
            }
            break;
        }
        self.current_writer.store(tid(), Relaxed);
        Ok(())
    }

    fn try_write_lock(&self) -> Result<(), Errno> {
        let prefer_writer = self.prefer_writer();
        match self.readers.fetch_update(Acquire, Relaxed, |state| {
            (!state.writer_locked()
                && (!state.has_readers() || (prefer_writer && state.write_phase())))
            .then(|| state.with_write_phase(true).with_writer_locked(true))
        }) {
            Ok(state) => {
                self.writers_futex
                    .store(PhaseWord::ZERO.with_phase(true), Relaxed);
                // Only a phase we started gets the futex word reset; a running phase may already carry waiters' used marker.
                if !state.write_phase() {
                    self.write_phase_futex
                        .store(PhaseWord::ZERO.with_phase(true), Relaxed);
                }
                self.current_writer.store(tid(), Relaxed);
                Ok(())
            }
            Err(_) => Err(Errno::BUSY),
        }
    }

    fn read_unlock(&self) {
        // The last reader out starts the write phase for a waiting primary writer and releases RWAITING waiters.
        fn release_reader(state: Readers) -> Readers {
            let mut next = state.decremented();
            if !next.has_readers() {
                if next.writer_locked() {
                    next = next.with_write_phase(true);
                }
                next = next.with_readers_waiting(false);
            }
            next
        }
        let (state, new_state) = self.readers.fetch_map(Release, Relaxed, release_reader);
        if new_state.write_phase() {
            // Explicit hand-over to the writer: the fence orders our futex store after the store of the reader that started this read phase.
            fence(Acquire);
            if self
                .write_phase_futex
                .swap(PhaseWord::ZERO.with_phase(true), Relaxed)
                .used()
            {
                futex_wake(&self.write_phase_futex, i32::MAX);
            }
        }
        if state.readers_waiting() != new_state.readers_waiting() {
            futex_wake(&self.readers, i32::MAX);
        }
    }

    fn write_unlock(&self) {
        self.current_writer.store(0, Relaxed);
        // Close the writers futex before deciding where the lock goes; the woken writer reopens it.
        let wake_writers = self.writers_futex.swap(PhaseWord::ZERO, Relaxed).used();
        if self.prefer_writer() {
            let mut writers = self.writers.load(Relaxed);
            while writers.raw_value() != 0 {
                // Release so the successor inherits our view of `readers`.
                match self.writers.compare_exchange_weak(
                    writers,
                    writers.with_handover(true),
                    Release,
                    Relaxed,
                ) {
                    Ok(_) => {
                        if wake_writers {
                            futex_wake(&self.writers_futex, 1);
                        }
                        return;
                    }
                    Err(actual) => writers = actual,
                }
            }
        }
        // No writer to hand over to: release WRLOCKED, ending the write phase only if readers are waiting to enter one.
        let (state, _) = self.readers.fetch_map(Release, Relaxed, |state| {
            state
                .with_writer_locked(false)
                .with_write_phase(if state.has_readers() {
                    !state.write_phase()
                } else {
                    state.write_phase()
                })
        });
        if state.has_readers() {
            // Explicit hand-over to the waiting readers.
            if self.write_phase_futex.swap(PhaseWord::ZERO, Relaxed).used() {
                futex_wake(&self.write_phase_futex, i32::MAX);
            }
        }
        if wake_writers {
            futex_wake(&self.writers_futex, 1);
        }
    }

    fn unlock(&self) {
        // A reader reads the zero the most recent writer stored here; only the writer itself can observe its own tid.
        if self.current_writer.load(Relaxed) == tid() {
            self.write_unlock();
        } else {
            self.read_unlock();
        }
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlock_rdlock(rwlock: &PthreadRwlock) -> c_int {
    signature_matches_libc!(libc::pthread_rwlock_rdlock(std::mem::transmute(rwlock)));
    match rwlock.read_lock() {
        Ok(()) => 0,
        Err(errno) => errno.into(),
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlock_tryrdlock(rwlock: &PthreadRwlock) -> c_int {
    signature_matches_libc!(libc::pthread_rwlock_tryrdlock(std::mem::transmute(rwlock)));
    match rwlock.try_read_lock() {
        Ok(()) => 0,
        Err(errno) => errno.into(),
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlock_wrlock(rwlock: &PthreadRwlock) -> c_int {
    signature_matches_libc!(libc::pthread_rwlock_wrlock(std::mem::transmute(rwlock)));
    match rwlock.write_lock() {
        Ok(()) => 0,
        Err(errno) => errno.into(),
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlock_trywrlock(rwlock: &PthreadRwlock) -> c_int {
    signature_matches_libc!(libc::pthread_rwlock_trywrlock(std::mem::transmute(rwlock)));
    match rwlock.try_write_lock() {
        Ok(()) => 0,
        Err(errno) => errno.into(),
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlock_unlock(rwlock: &PthreadRwlock) -> c_int {
    signature_matches_libc!(libc::pthread_rwlock_unlock(std::mem::transmute(rwlock)));
    rwlock.unlock();
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlock_init(
    rwlock: &mut PthreadRwlock,
    attr: Option<NonNull<PthreadRwlockAttr>>,
) -> c_int {
    signature_matches_libc!(libc::pthread_rwlock_init(
        std::mem::transmute(rwlock),
        std::mem::transmute(attr)
    ));
    let (kind, pshared) = attr
        .map(|attr| {
            // SAFETY: `attr` is `Some` only when the caller passed a valid, aligned, initialized attr pointer.
            let attr = unsafe { attr.as_ref() };
            (attr.lockkind, attr.pshared)
        })
        .unwrap_or((PthreadLockKind::PreferReader, PthreadProcessShared::Private));
    *rwlock = PthreadRwlock::new(kind, pshared);
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlock_destroy(rwlock: &PthreadRwlock) -> c_int {
    signature_matches_libc!(libc::pthread_rwlock_destroy(std::mem::transmute(rwlock)));
    0
}

/// glibc's `pthread_rwlockattr_t { int lockkind; int pshared; }`.
#[repr(C)]
struct PthreadRwlockAttr {
    lockkind: PthreadLockKind,
    pshared: PthreadProcessShared,
}

const _: () = assert!(size_of::<PthreadRwlockAttr>() == size_of::<libc::pthread_rwlockattr_t>());

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlockattr_init(attr: &mut PthreadRwlockAttr) -> c_int {
    signature_matches_libc!(libc::pthread_rwlockattr_init(std::mem::transmute(attr)));
    *attr = PthreadRwlockAttr {
        lockkind: PthreadLockKind::PreferReader,
        pshared: PthreadProcessShared::Private,
    };
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlockattr_destroy(attr: &PthreadRwlockAttr) -> c_int {
    signature_matches_libc!(libc::pthread_rwlockattr_destroy(std::mem::transmute(attr)));
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlockattr_setpshared(
    attr: &mut PthreadRwlockAttr,
    pshared: c_int,
) -> c_int {
    signature_matches_libc!(libc::pthread_rwlockattr_setpshared(
        std::mem::transmute(attr),
        pshared
    ));
    let Ok(pshared) = PthreadProcessShared::try_from(pshared) else {
        return Errno::INVAL.into();
    };
    attr.pshared = pshared;
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlockattr_getpshared(
    attr: &PthreadRwlockAttr,
    pshared: &mut c_int,
) -> c_int {
    signature_matches_libc!(libc::pthread_rwlockattr_getpshared(
        std::mem::transmute(attr),
        std::mem::transmute(pshared)
    ));
    *pshared = attr.pshared as c_int;
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlockattr_setkind_np(
    attr: &mut PthreadRwlockAttr,
    preference: c_int,
) -> c_int {
    signature_matches_libc!(libc::pthread_rwlockattr_setkind_np(
        std::mem::transmute(attr),
        preference
    ));
    let Ok(kind) = PthreadLockKind::try_from(preference) else {
        return Errno::INVAL.into();
    };
    attr.lockkind = kind;
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_rwlockattr_getkind_np(
    attr: &PthreadRwlockAttr,
    preference: &mut c_int,
) -> c_int {
    signature_matches_libc!(libc::pthread_rwlockattr_getkind_np(
        std::mem::transmute(attr),
        std::mem::transmute(preference)
    ));
    *preference = attr.lockkind as c_int;
    0
}
