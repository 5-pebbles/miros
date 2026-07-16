use std::{
    ffi::c_void,
    mem::size_of,
    ptr::{self, null_mut, NonNull},
};

use super::{attr::PthreadAttr, PthreadT};
use crate::{
    libc::{
        mem::{mmap, mprotect, MapFlags, ProtectionFlags},
        process::clone::{clone3, Clone3Args, Clone3Flags},
    },
    page_size, signature_matches_libc,
    syscall::thread_pointer::get_thread_pointer,
    tls::{
        get_tls_allocator,
        thread_control_block::{
            AtomicDetachState, DetachState, DynamicThreadVector, ThreadControlBlock,
        },
        TLS_RESERVE_SIZE,
    },
};

const DEFAULT_STACK_SIZE: usize = 8 * 1024 * 1024;

/// Recover a worker thread's stack bounds from its TCB — the inverse of the region built below.
/// `[guard][stack][TLS_RESERVE][TCB][miros tls]`: the TCB address (= thread pointer) sits one `TLS_RESERVE_SIZE` above the stack top, and the stack starts one guard page into the region.
pub unsafe fn thread_stack_bounds(
    thread_control_block: *const ThreadControlBlock,
) -> (usize, usize) {
    let (region_base, _) = (*thread_control_block).region.to_raw_parts();
    let stack_base = region_base.addr() + page_size::get_page_size();
    let stack_size = (thread_control_block as usize)
        .saturating_sub(TLS_RESERVE_SIZE)
        .saturating_sub(stack_base);
    (stack_base, stack_size)
}

unsafe extern "C" fn pthread_entry(context: *mut c_void) -> ! {
    let context = &*(context as *const PthreadContext);
    crate::allocator::install_heap();

    // Userland:
    let return_value = (context.entry_function)(context.entry_argument);

    let thread_pointer = get_thread_pointer() as *mut ThreadControlBlock;
    (*thread_pointer).return_value = return_value;

    // Destructors run before `abandon_heap` because they may malloc/free.
    super::run_at_thread_exit_destructors();
    crate::allocator::abandon_heap();
    super::self_detach::on_thread_exit(thread_pointer);
}

#[repr(C)]
struct PthreadContext {
    entry_function: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    entry_argument: *mut c_void,
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_create(
    thread_addr_out: &mut PthreadT,
    attr: Option<NonNull<PthreadAttr>>,
    entry_function: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    entry_argument: *mut c_void,
) -> i32 {
    signature_matches_libc!(libc::pthread_create(
        std::mem::transmute(thread_addr_out),
        std::mem::transmute(attr),
        std::mem::transmute(entry_function),
        entry_argument,
    ));

    let resolved_attr = super::attr::resolve(attr, DEFAULT_STACK_SIZE, page_size::get_page_size());
    let guard_size = resolved_attr.guard_size;
    let stack_size = resolved_attr.stack_size;

    let miros_tls_size;
    {
        let allocator = get_tls_allocator().lock().unwrap_unchecked();
        miros_tls_size = allocator
            .miros_template()
            .map(|t| t.block_size)
            .unwrap_or(0);
    }

    let total_size = guard_size
        + stack_size
        + TLS_RESERVE_SIZE
        + size_of::<ThreadControlBlock>()
        + miros_tls_size;

    let region = mmap(
        ptr::null_mut(),
        total_size,
        ProtectionFlags::ZERO
            .with_readable(true)
            .with_writable(true),
        MapFlags::ZERO.with_private(true).with_anonymous(true),
        -1,
        0,
    );

    mprotect(region, guard_size, ProtectionFlags::ZERO);

    let thread_pointer = region.add(guard_size + stack_size + TLS_RESERVE_SIZE) as *mut c_void;
    let thread_control_block = thread_pointer as *mut ThreadControlBlock;

    let initial_detach_state = if resolved_attr.detached {
        DetachState::Detached
    } else {
        DetachState::Joinable
    };
    let current_tcb = get_thread_pointer() as *const ThreadControlBlock;
    *thread_control_block = ThreadControlBlock {
        thread_pointee: [],
        thread_pointer_register: thread_pointer,
        tid: 0,
        detach_state: AtomicDetachState::new(initial_detach_state),
        return_value: null_mut(),
        region: ptr::slice_from_raw_parts_mut(region, total_size),
        canary: (*current_tcb).canary,
        dynamic_thread_vector: DynamicThreadVector::new(),
    };

    get_tls_allocator()
        .lock()
        .unwrap_unchecked()
        .initialize_thread_tls(thread_pointer);

    let tid_pointer = ptr::addr_of_mut!((*thread_control_block).tid);
    let child_stack = region.add(guard_size);

    let context = child_stack as *mut PthreadContext;
    *context = PthreadContext {
        entry_function,
        entry_argument,
    };

    let clone_args = Clone3Args {
        flags: Clone3Flags::ZERO
            .with_share_virtual_memory(true)
            .with_share_filesystem_info(true)
            .with_share_file_descriptors(true)
            .with_share_signal_handlers(true)
            .with_thread(true)
            .with_share_sysvsem(true)
            .with_set_tls(true)
            .with_parent_set_tid(true)
            .with_child_clear_tid(true),
        pid_file_descriptor: ptr::null_mut(),
        child_tid_pointer: tid_pointer,
        parent_tid_pointer: tid_pointer,
        exit_signal: 0,
        child_stack,
        child_stack_size: stack_size as u64,
        thread_local_storage: thread_pointer as *mut u8,
        set_tid_array: ptr::null_mut(),
        set_tid_array_count: 0,
        target_control_group: 0,
    };

    let result = clone3(&clone_args, pthread_entry, context as *mut c_void);
    if result < 0 {
        crate::libc::mem::munmap(region, total_size);
        return libc::EAGAIN;
    }

    *thread_addr_out = thread_pointer as PthreadT;
    0
}
