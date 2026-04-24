use std::{
    ops::Not,
    os::fd::{AsRawFd, BorrowedFd},
};

use bitbybit::bitenum;

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
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

#[cfg_attr(not(test), no_mangle)]
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

    let result = syscall!(
        Syscall::FCntl,
        file_descriptor.as_raw_fd(),
        command.raw_value(),
        argument
    );

    if result < 0 {
        set_errno(Errno(result.unsigned_abs() as u32));
        -1
    } else {
        result as i32
    }
}
