#[repr(usize)]
pub enum Syscall {
    Read = 0,
    Write = 1,
    Close = 3,
    Stat = 4,
    FStat = 5,
    Mmap = 9,
    Munmap = 11,
    GetPid = 39,
    Exit = 60,
    ArchPrctl = 158,
    GetTid = 186,
    TgKill = 234,
    OpenAt = 257,
}

pub mod exit;
pub mod thread_pointer;
