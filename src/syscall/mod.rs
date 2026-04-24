#[repr(usize)]
#[cfg(target_arch = "x86_64")]
pub enum Syscall {
    Read = 0,
    PRead64 = 17,
    Write = 1,
    Close = 3,
    FCntl = 72,
    Stat = 4,
    FStat = 5,
    Mmap = 9,
    Mprotect = 10,
    Munmap = 11,
    GetPid = 39,
    Exit = 60,
    ArchPrctl = 158,
    GetTid = 186,
    TgKill = 234,
    OpenAt = 257,
    GetRandom = 318,
}

// TT-muncher: peels one register constraint and one argument per recursion step,
// accumulating `in("reg") value` operands into a single `asm!` block.
macro_rules! syscall {
    ($syscall:expr $(, $args:expr)* $(,)?) => {
        #[cfg(target_arch = "x86_64")]
        syscall!(
            @build $syscall,
            [in("rdi"), in("rsi"), in("rdx"), in("r10"), in("r8"), in("r9"),],
            []
            $(, $args)*
        )
    };
    (@build $syscall:expr, [$($unused:tt)*], [$($operands:tt)*]) => {{
        let result: isize;
        #[cfg(target_arch = "x86_64")]
        std::arch::asm!(
            "syscall",
            inlateout("rax") $syscall as usize => result,
            $($operands)*
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags),
        );
        result
    }};
    (@build $syscall:expr, [$constraint:tt $register:tt, $($rest:tt)*], [$($operands:tt)*], $arg:expr $(, $more:expr)*) => {
        syscall!(
            @build $syscall,
            [$($rest)*],
            [$($operands)* $constraint $register $arg as usize,]
            $(, $more)*
        )
    };
    (@build $syscall:expr, [], [$($operands:tt)*], $($overflow:expr),+) => {
        compile_error!("x86_64 syscall ABI supports at most 6 arguments")
    };
}

pub(crate) use syscall;

pub mod exit;
pub mod thread_pointer;
