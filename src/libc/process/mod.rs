mod libc_start_main;

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn _dl_fini() {}

use std::{arch::asm, cell::Cell, io, io::Write, process, thread};

use crate::{signature_matches_libc, syscall::Syscall};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn getpid() -> i32 {
    signature_matches_libc!(std::mem::transmute(libc::getpid()));
    let result: usize;

    #[cfg(target_arch = "x86_64")]
    {
        asm!(
            "syscall",
            inlateout("rax") Syscall::GetPid as usize => result,
            out("rcx") _,
            out("r11") _,
            options(nostack),
        )
    }
    result as i32
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn raise(signal_number: i32) -> i32 {
    signature_matches_libc!(libc::raise(signal_number));

    let process_id = process::id();
    let thread_id = thread::current().id();

    let result: isize;
    #[cfg(target_arch = "x86_64")]
    {
        asm!(
            "syscall",
            inlateout("rax") Syscall::TgKill as usize => result,
            in("rdi") process_id,
            in("rsi") thread_id.as_u64().get(),
            in("rdx") signal_number,
            out("rcx") _,
            out("r11") _,
            options(nostack),
        )
    }
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
