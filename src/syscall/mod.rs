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
    MMap = 9,
    IoCtl = 16,
    MProtect = 10,
    MunMap = 11,
    MreMap = 25,
    GetPid = 39,
    Clone = 56,
    Exit = 60,
    ArchPrCtl = 158,
    Futex = 202,
    GetTid = 186,
    TgKill = 234,
    OpenAt = 257,
    GetRandom = 318,
    Clone3 = 435,
    GetTimeOfDay = 96,
    ClockGetTime = 228,
    SchedGetAffinity = 204,
    PrCtl = 157,
    RtSigAction = 13,
    RtSigReturn = 15,
    SigAltStack = 131,
    Poll = 7,
    EpollCreate1 = 291,
    EpollCtl = 233,
    EpollWait = 232,
    Eventfd2 = 290,
    SocketPair = 53,
    MAdvise = 28,
    SchedYield = 24,
    GetCwd = 79,
    Statx = 332,
    LSeek = 8,
    GetDents64 = 217,
    Socket = 41,
    Bind = 49,
    Listen = 50,
    Accept4 = 288,
    SetSockOpt = 54,
    GetSockOpt = 55,
    GetSockName = 51,
    GetPeerName = 52,
    RecvFrom = 45,
    SendTo = 44,
    WriteV = 20,
    Shutdown = 48,
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
pub mod futex;
pub mod thread_pointer;
