use std::{cell::RefCell, ffi::c_void};

use crate::{
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

mod create;
mod join;
mod key;

pub(crate) use key::run_key_destructors;

struct TlsDestructor {
    function: unsafe extern "C" fn(*mut c_void),
    object: *mut c_void,
}

#[thread_local]
static TLS_DESTRUCTORS: RefCell<Vec<TlsDestructor>> = RefCell::new(Vec::new());

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn __cxa_thread_atexit_impl(
    destructor: unsafe extern "C" fn(*mut c_void),
    object: *mut c_void,
    _dso_symbol: *mut c_void,
) -> i32 {
    let mut destructors = TLS_DESTRUCTORS.borrow_mut();
    if destructors.try_reserve(1).is_err() {
        return -1;
    }
    destructors.push(TlsDestructor {
        function: destructor,
        object,
    });
    0
}

/// Drains this thread's `thread_local` destructors in LIFO order; a destructor registering another lands on the stack and runs in the same drain.
pub unsafe fn call_tls_destructors() {
    // `let else` ends the borrow before the call, so destructors can re-enter registration.
    loop {
        let Some(TlsDestructor { function, object }) = TLS_DESTRUCTORS.borrow_mut().pop() else {
            break;
        };
        function(object);
    }
    // Nothing runs drop for `#[thread_local]` statics, and `abandon_heap` would pin the buffer in the abandoned heap.
    TLS_DESTRUCTORS.take();
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn gettid() -> i32 {
    signature_matches_libc!(std::mem::transmute(libc::gettid()));
    let result = syscall!(Syscall::GetTid);
    result as i32
}
