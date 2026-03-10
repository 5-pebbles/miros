use std::{
    arch::asm,
    ops::Not,
    os::fd::{AsRawFd, BorrowedFd},
};

use bitbybit::bitenum;

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::Syscall,
};

#[repr(u32)]
#[bitenum(u32, exhaustive = false)]
pub enum FCntlCommand {
    DuplicateFileDescriptor = 0,
    GetCloseOnExec = 1,
    SetCloseOnExec = 2,
    GetOpenFlags = 3,
    SetOpenFlags = 4,
    DuplicateFileDescriptorCloseOnExec = 1030,
}

#[no_mangle]
unsafe extern "C" fn fcntl(
    file_descriptor: BorrowedFd<'_>,
    command: FCntlCommand,
    mut arguments: ...
) -> i32 {
    signature_matches_libc!(libc::fcntl(
        std::mem::transmute(file_descriptor),
        std::mem::transmute(command),
    ));

    let argument: usize = matches!(
        command,
        FCntlCommand::GetCloseOnExec | FCntlCommand::GetOpenFlags
    )
    .not()
    .then_some(arguments.arg())
    .unwrap_or_default();

    let result: isize;
    #[cfg(target_arch = "x86_64")]
    {
        asm!(
            "syscall",
            inlateout("rax") Syscall::FCntl as usize => result,
            in("rdi") file_descriptor.as_raw_fd(),
            in("rsi") command.raw_value(),
            in("rdx") argument,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }

    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        result as i32
    }
}
