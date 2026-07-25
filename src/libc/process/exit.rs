use std::{ffi::c_void, ptr, sync::Mutex};

use crate::{
    libc::stdio::flush_all_streams, signature_matches_libc, syscall::exit::exit as exit_syscall,
};

enum HandlerKind {
    Plain(extern "C" fn()),
    WithObject(unsafe extern "C" fn(*mut c_void)),
}

struct ExitHandler {
    kind: HandlerKind,
    object: *mut c_void,
}

// The object pointer is opaque to us; the registering caller owns its validity through exit.
unsafe impl Send for ExitHandler {}

static EXIT_HANDLERS: Mutex<Vec<ExitHandler>> = Mutex::new(Vec::new());

fn register(handler: ExitHandler) -> i32 {
    let Ok(mut handlers) = EXIT_HANDLERS.lock() else {
        return -1;
    };
    if handlers.try_reserve(1).is_err() {
        return -1;
    }
    handlers.push(handler);
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn atexit(function: extern "C" fn()) -> i32 {
    signature_matches_libc!(libc::atexit(function));
    register(ExitHandler {
        kind: HandlerKind::Plain(function),
        object: ptr::null_mut(),
    })
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn __cxa_atexit(
    function: unsafe extern "C" fn(*mut c_void),
    object: *mut c_void,
    _dso_handle: *mut c_void,
) -> i32 {
    register(ExitHandler {
        kind: HandlerKind::WithObject(function),
        object,
    })
}

/// Run registered handlers LIFO, then flush every stream.
pub(crate) unsafe fn run_exit_sequence() {
    // Pop with the lock released across each call, so a handler may register more or call `exit`.
    loop {
        let Some(handler) = EXIT_HANDLERS
            .lock()
            .ok()
            .and_then(|mut handlers| handlers.pop())
        else {
            break;
        };
        match handler.kind {
            HandlerKind::Plain(function) => function(),
            HandlerKind::WithObject(function) => function(handler.object),
        }
    }
    flush_all_streams();
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn exit(code: i32) -> ! {
    signature_matches_libc!(libc::exit(code));
    run_exit_sequence();
    exit_syscall(code as usize);
}
