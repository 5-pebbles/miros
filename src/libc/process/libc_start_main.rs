use std::ffi::c_void;

use crate::syscall::exit::exit;

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn __libc_start_main(
    main: unsafe extern "C" fn(i32, *const *const u8, *const *const u8) -> i32,
    argc: i32,
    argv: *const *const u8,
    _init: *const c_void,
    _fini: *const c_void,
    rtld_fini: Option<unsafe extern "C" fn()>,
    _stack_end: *const c_void,
) -> ! {
    let envp = argv.offset(argc as isize + 1);
    let exit_code = main(argc, argv, envp);

    // Matching glibc's exit: the calling thread's `thread_local` dtors run before the atexit chain; key destructors don't run for the main thread.
    crate::libc::threads::call_tls_destructors();

    super::exit::run_exit_sequence();

    // TODO: Register rtld_fini (fini array functions) to run at exit.
    if let Some(rtld_fini) = rtld_fini {
        rtld_fini();
    }

    exit(exit_code as usize);
}
