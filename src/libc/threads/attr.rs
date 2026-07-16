use core::{
    ffi::{c_int, c_void},
    ptr::NonNull,
};

use crate::{
    libc::threads::PthreadT, page_size, signature_matches_libc,
    tls::thread_control_block::ThreadControlBlock,
};

const PTHREAD_CREATE_JOINABLE: i32 = 0;
const PTHREAD_CREATE_DETACHED: i32 = 1;

/// A `0` field means "use the runtime default", so a zeroed (`pthread_attr_init`) blob is all-defaults.
#[repr(C, align(8))]
pub struct PthreadAttr {
    stack_base: *mut c_void,
    stack_size: usize,
    guard_size: usize,
    detach_state: i32,
    _reserved: [u8; 28],
}

const _: () = assert!(size_of::<PthreadAttr>() == size_of::<libc::pthread_attr_t>());
const _: () = assert!(align_of::<PthreadAttr>() == align_of::<libc::pthread_attr_t>());

/// Thread-creation parameters resolved from a (possibly null) attr against the runtime defaults.
pub struct ResolvedAttr {
    pub stack_size: usize,
    pub guard_size: usize,
    pub detached: bool,
}

pub unsafe fn resolve(
    attr: Option<NonNull<PthreadAttr>>,
    default_stack_size: usize,
    default_guard_size: usize,
) -> ResolvedAttr {
    match attr.map(|attr| attr.as_ref()) {
        None => ResolvedAttr {
            stack_size: default_stack_size,
            guard_size: default_guard_size,
            detached: false,
        },
        Some(attr) => ResolvedAttr {
            stack_size: if attr.stack_size != 0 {
                attr.stack_size
            } else {
                default_stack_size
            },
            guard_size: if attr.guard_size != 0 {
                attr.guard_size
            } else {
                default_guard_size
            },
            detached: attr.detach_state == PTHREAD_CREATE_DETACHED,
        },
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_attr_init(attr: &mut PthreadAttr) -> c_int {
    signature_matches_libc!(libc::pthread_attr_init(std::mem::transmute(attr)));
    *attr = PthreadAttr {
        stack_base: core::ptr::null_mut(),
        stack_size: 0,
        guard_size: page_size::get_page_size(),
        detach_state: PTHREAD_CREATE_JOINABLE,
        _reserved: [0; 28],
    };
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_attr_destroy(attr: &PthreadAttr) -> c_int {
    signature_matches_libc!(libc::pthread_attr_destroy(std::mem::transmute(attr)));
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_attr_setdetachstate(
    attr: &mut PthreadAttr,
    detach_state: c_int,
) -> c_int {
    signature_matches_libc!(libc::pthread_attr_setdetachstate(
        std::mem::transmute(attr),
        detach_state
    ));
    if detach_state != PTHREAD_CREATE_JOINABLE && detach_state != PTHREAD_CREATE_DETACHED {
        return libc::EINVAL;
    }
    attr.detach_state = detach_state;
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_attr_setstacksize(attr: &mut PthreadAttr, stack_size: usize) -> c_int {
    signature_matches_libc!(libc::pthread_attr_setstacksize(
        std::mem::transmute(attr),
        stack_size
    ));
    attr.stack_size = stack_size;
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_attr_getstack(
    attr: &PthreadAttr,
    stack_base_out: &mut *mut c_void,
    stack_size_out: &mut usize,
) -> c_int {
    signature_matches_libc!(libc::pthread_attr_getstack(
        std::mem::transmute(attr),
        std::mem::transmute(stack_base_out),
        std::mem::transmute(stack_size_out)
    ));
    *stack_base_out = attr.stack_base;
    *stack_size_out = attr.stack_size;
    0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_attr_getguardsize(
    attr: &PthreadAttr,
    guard_size_out: &mut usize,
) -> c_int {
    signature_matches_libc!(libc::pthread_attr_getguardsize(
        std::mem::transmute(attr),
        std::mem::transmute(guard_size_out)
    ));
    *guard_size_out = attr.guard_size;
    0
}

/// Reports a running thread's stack from its TCB region. The handle is the thread pointer (= TCB address),
/// and a worker's region is `[guard][stack][TLS_RESERVE][TCB][miros tls]`, so the stack is the slice below it.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_getattr_np(thread: PthreadT, attr: &mut PthreadAttr) -> c_int {
    signature_matches_libc!(libc::pthread_getattr_np(
        thread as _,
        std::mem::transmute(attr)
    ));
    let thread_control_block = thread as *const ThreadControlBlock;
    let (stack_base, stack_size) = super::create::thread_stack_bounds(thread_control_block);

    *attr = PthreadAttr {
        stack_base: stack_base as *mut c_void,
        stack_size,
        guard_size: page_size::get_page_size(),
        detach_state: PTHREAD_CREATE_JOINABLE,
        _reserved: [0; 28],
    };
    0
}
