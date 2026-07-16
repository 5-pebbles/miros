use core::ffi::c_int;
use std::sync::atomic::{AtomicU32, Ordering};

use strum::FromRepr;

use crate::{
    libc::threads::{futex_wait, futex_wake},
    signature_matches_libc,
};

#[derive(FromRepr, PartialEq, Clone, Copy)]
#[repr(u32)]
enum OnceState {
    NotRun = 0,
    InProgress = 1,
    Done = 2,
}

/// The 4-byte `pthread_once_t` blob is the futex word directly; `PTHREAD_ONCE_INIT` is `NotRun`.
#[repr(transparent)]
struct PthreadOnce(AtomicU32);

const _: () = assert!(size_of::<PthreadOnce>() == size_of::<libc::pthread_once_t>());

impl PthreadOnce {
    fn state(&self) -> OnceState {
        OnceState::from_repr(self.0.load(Ordering::Acquire)).unwrap()
    }

    /// Claim the right to run the routine (`NotRun -> InProgress`); on failure, the state we lost to.
    fn try_begin(&self) -> Result<(), OnceState> {
        self.0
            .compare_exchange(
                OnceState::NotRun as u32,
                OnceState::InProgress as u32,
                Ordering::Acquire,
                Ordering::Acquire,
            )
            .map(|_| ())
            .map_err(|value| OnceState::from_repr(value).unwrap())
    }

    fn complete(&self) {
        self.0.store(OnceState::Done as u32, Ordering::Release);
        futex_wake(&self.0, i32::MAX);
    }

    fn wait_for_completion(&self) {
        futex_wait(&self.0, OnceState::InProgress as u32);
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_once(once: &PthreadOnce, init_routine: extern "C" fn()) -> c_int {
    signature_matches_libc!(libc::pthread_once(
        std::mem::transmute(once),
        std::mem::transmute(init_routine)
    ));
    if once.state() == OnceState::Done {
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
            Err(OnceState::Done) => return 0,
            Err(_) => once.wait_for_completion(),
        }
    }
}
