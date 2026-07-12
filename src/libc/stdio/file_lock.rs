use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering},
};

use arbitrary_int::u31;
use bitbybit::bitfield;

use crate::{
    syscall::{futex::FutexOperation, syscall, thread_pointer::get_thread_pointer, Syscall},
    tls::thread_control_block::ThreadControlBlock,
};

/// The futex word: owner tid in the low 31 bits, `has_waiters` in the top bit (the kernel's `FUTEX_WAITERS` slot).
/// A raw `0` is free. Ownership and futex state share one word, so claiming ownership *is* the acquiring CAS.
#[bitfield(u32)]
struct LockWord {
    #[bits(0..=30, rw)]
    owner: u31,
    #[bit(31, rw)]
    has_waiters: bool,
}

impl LockWord {
    /// This word's raw value with the waiters bit set.
    fn contended(self) -> u32 {
        self.with_has_waiters(true).raw_value()
    }
}

/// Recursive, tid-keyed stream lock behind `flockfile`/`funlockfile`.
pub struct FileLock {
    word: AtomicU32,
    recursion: UnsafeCell<u32>,
}

// SAFETY: `recursion` is only touched by the thread whose tid is in `word`.
unsafe impl Send for FileLock {}
unsafe impl Sync for FileLock {}

impl FileLock {
    pub const fn new() -> Self {
        Self {
            word: AtomicU32::new(0),
            recursion: UnsafeCell::new(0),
        }
    }

    pub fn lock(&self) {
        let tid =
            u31::new(unsafe { (*get_thread_pointer().cast::<ThreadControlBlock>()).tid } as u32);
        let held = LockWord::ZERO.with_owner(tid).raw_value();

        match self
            .word
            .compare_exchange(0, held, Ordering::Acquire, Ordering::Relaxed)
        {
            // Uncontended: the word was free and is now ours.
            Ok(_) => unsafe { *self.recursion.get() = 1 },
            // Already ours.
            Err(current) if LockWord::new_with_raw_value(current).owner() == tid => unsafe {
                *self.recursion.get() += 1
            },
            // Held by another thread.
            Err(_) => {
                self.acquire_contended(tid);
                unsafe { *self.recursion.get() = 1 };
            }
        }
    }

    pub fn unlock(&self) {
        // SAFETY: only the owner reaches here, so `recursion` is exclusively ours.
        let recursion = unsafe { &mut *self.recursion.get() };
        *recursion -= 1;
        if *recursion != 0 {
            return;
        }

        if LockWord::new_with_raw_value(self.word.swap(0, Ordering::Release)).has_waiters() {
            self.futex_wake(1);
        }
    }

    /// `compare_exchange` on the word, relaxed on failure; `true` if it swapped.
    fn cas(&self, current: u32, new: u32, success: Ordering) -> bool {
        self.word
            .compare_exchange(current, new, success, Ordering::Relaxed)
            .is_ok()
    }

    /// Spin the CAS/futex loop until we win.
    fn acquire_contended(&self, tid: u31) {
        let held_contended = LockWord::ZERO.with_owner(tid).contended();

        loop {
            match self.word.load(Ordering::Relaxed) {
                0 =>
                // Free — install ourselves and we're done.
                {
                    if self.cas(0, held_contended, Ordering::Acquire) {
                        return;
                    }
                }
                current =>
                // Held — publish the waiters bit if needed, then sleep on that value.
                {
                    let contended = LockWord::new_with_raw_value(current).contended();
                    if contended == current || self.cas(current, contended, Ordering::Relaxed) {
                        self.futex_wait(contended);
                    }
                }
            }
        }
    }

    fn futex_wait(&self, expected: u32) {
        unsafe {
            syscall!(
                Syscall::Futex,
                self.word.as_ptr(),
                FutexOperation::Wait,
                expected,
                0usize,
                0usize,
                0usize
            );
        }
    }

    fn futex_wake(&self, count: u32) {
        unsafe {
            syscall!(
                Syscall::Futex,
                self.word.as_ptr(),
                FutexOperation::Wake,
                count,
                0usize,
                0usize,
                0usize
            );
        }
    }
}
