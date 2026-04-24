mod libc_start_main;

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn rtld_fini() {}

use std::{arch::asm, cell::Cell, io, io::Write, process, thread};

use crate::{
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn getpid() -> i32 {
    signature_matches_libc!(std::mem::transmute(libc::getpid()));

    let result = syscall!(Syscall::GetPid);
    result as i32
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn raise(signal_number: i32) -> i32 {
    signature_matches_libc!(libc::raise(signal_number));

    let process_id = process::id();
    let thread_id = thread::current().id();

    let result = syscall!(
        Syscall::TgKill,
        process_id,
        thread_id.as_u64().get(),
        signal_number
    );
    result as i32
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn abort() -> ! {
    #[thread_local]
    static ABORT_IN_PROGRESS: Cell<bool> = const { Cell::new(false) };

    // SAFETY: We must make sure this function is not called recursively.
    if ABORT_IN_PROGRESS.replace(true) {
        // I think I was called recursively; bye. o7
        asm!("ud2", options(noreturn, nostack));
    }

    // Flush stdio streams per POSIX requirements:
    let _ = io::stdout().flush();
    let _ = io::stderr().flush();

    unsafe {
        raise(libc::SIGABRT);
    }

    // SAFETY: POSIX states that we should unregister the sigabort handler and try again...
    // But why the fuck would you return normally from a sigabort?!?
    // If you do, you're fucking retarded and have an invalid instruction coming your way. ┗(▀̿ĺ̯▀̿ ̿)┓  ●~*

    asm!("ud2", options(noreturn, nostack));
}
