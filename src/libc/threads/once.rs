use core::ffi::c_int;
use std::sync::atomic::Ordering::{Acquire, Release};

use atomic::Atomic;
use bitbybit::bitenum;

use crate::{
    libc::{
        errno::Errno,
        threads::{futex_wait, futex_wake},
    },
    signature_matches_libc,
};

type OnceWordParse = Result<OnceWord, u32>;

#[bitenum(u32)]
#[derive(PartialEq, Eq)]
enum OnceWord {
    NotRun = 0,
    InProgress = 1,
    Done = 2,
}

/// The 4-byte `pthread_once_t` blob is the futex word directly.
#[repr(transparent)]
struct PthreadOnce(Atomic<u32>);

const _: () = assert!(size_of::<PthreadOnce>() == size_of::<libc::pthread_once_t>());

impl PthreadOnce {
    fn load(&self) -> OnceWordParse {
        OnceWord::new_with_raw_value(self.0.load(Acquire))
    }

    /// Claim the right to run the routine; on failure, the word we lost to.
    fn try_begin(&self) -> Result<(), OnceWordParse> {
        self.0
            .compare_exchange(
                OnceWord::NotRun.raw_value(),
                OnceWord::InProgress.raw_value(),
                Acquire,
                Acquire,
            )
            .map(|_| ())
            .map_err(|err| OnceWord::new_with_raw_value(err))
    }

    fn complete(&self) {
        self.0.store(OnceWord::Done.raw_value(), Release);
        futex_wake(&self.0, i32::MAX);
    }

    fn wait_for_completion(&self) {
        futex_wait(&self.0, OnceWord::InProgress.raw_value());
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_once(once: &PthreadOnce, init_routine: extern "C" fn()) -> c_int {
    signature_matches_libc!(libc::pthread_once(
        std::mem::transmute(once),
        std::mem::transmute(init_routine)
    ));
    if once.load() == Ok(OnceWord::Done) {
        return 0;
    }

    loop {
        match once.try_begin() {
            // Winner: `panic = abort` guarantees we reach `complete`, so there is no cancel/reset path.
            Ok(()) => {
                init_routine();
                once.complete();
                return 0;
            }
            Err(Ok(OnceWord::NotRun)) => unreachable!(),
            Err(Ok(OnceWord::InProgress)) => once.wait_for_completion(),
            Err(Err(_)) => return Errno::INVAL.into(),
            Err(Ok(OnceWord::Done)) => return 0,
        }
    }
}
