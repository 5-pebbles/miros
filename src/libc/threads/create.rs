use std::{
    ffi::c_void,
    mem::size_of,
    ptr::{self, null_mut},
};

use crate::{
    libc::{
        mem::{mmap, mprotect, MapFlags, ProtectionFlags},
        process::clone::{clone3, Clone3Args, Clone3Flags},
    },
    page_size, signature_matches_libc,
    syscall::{exit, thread_pointer::get_thread_pointer},
    tls::{
        get_tls_allocator,
        thread_control_block::{DynamicThreadVector, ThreadControlBlock},
        TLS_RESERVE_SIZE,
    },
};

const DEFAULT_STACK_SIZE: usize = 8 * 1024 * 1024;

type PthreadT = usize;
type PthreadAttrT = *const c_void;

unsafe extern "C" fn pthread_entry(context: *mut c_void) -> ! {
    let context = &*(context as *const PthreadContext);
    crate::allocator::install_heap();

    // Userland:
    let return_value = (context.entry_function)(context.entry_argument);

    let thread_pointer = get_thread_pointer() as *mut ThreadControlBlock;
    (*thread_pointer).return_value = return_value;

    // glibc teardown order: `thread_local` dtors, key dtors, then the allocator cause dtors may malloc/free.
    // A key destructor may register new `thread_local` dtors via `__cxa_thread_atexit_impl`; re-drain until quiescent.
    loop {
        super::call_tls_destructors();
        if !super::run_key_destructors() {
            break;
        }
    }
    crate::allocator::abandon_heap();
    exit::exit(0);
}

#[repr(C)]
struct PthreadContext {
    entry_function: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    entry_argument: *mut c_void,
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_create(
    thread_addr_out: *mut PthreadT,
    _attr: PthreadAttrT,
    entry_function: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    entry_argument: *mut c_void,
) -> i32 {
    signature_matches_libc!(libc::pthread_create(
        thread_addr_out as *mut _,
        _attr as *const _,
        std::mem::transmute(entry_function),
        entry_argument,
    ));

    let guard_size = page_size::get_page_size();
    let stack_size = DEFAULT_STACK_SIZE;

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

    let current_tcb = get_thread_pointer() as *const ThreadControlBlock;
    *thread_control_block = ThreadControlBlock {
        thread_pointee: [],
        thread_pointer_register: thread_pointer,
        tid: 0,
        _padding: Default::default(),
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
