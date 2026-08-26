use std::os::fd::{AsRawFd, BorrowedFd};

use bitbybit::bitenum;

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc, syscall,
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

impl FCntlCommand {
    /// Commands that take / read the variadic slot.
    pub fn takes_third_argument(&self) -> bool {
        match self {
            FCntlCommand::GetCloseOnExec | FCntlCommand::GetOpenFlags => false,
            FCntlCommand::DuplicateFileDescriptor
            | FCntlCommand::SetCloseOnExec
            | FCntlCommand::SetOpenFlags
            | FCntlCommand::DuplicateFileDescriptorCloseOnExec => true,
        }
    }
}

unsafe fn fcntl_dispatch(
    file_descriptor: BorrowedFd<'_>,
    command: FCntlCommand,
    argument: usize,
) -> i32 {
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

    let argument: usize = command
        .takes_third_argument()
        .then_some(arguments.next_arg())
        .unwrap_or_default();
    fcntl_dispatch(file_descriptor, command, argument)
}

// LFS alias: on x86_64 file offsets are already 64-bit, so `fcntl64` is `fcntl` verbatim.
#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn fcntl64(
    file_descriptor: BorrowedFd<'_>,
    command: FCntlCommand,
    mut arguments: ...
) -> i32 {
    signature_matches_libc!(libc::fcntl(
        std::mem::transmute(file_descriptor),
        std::mem::transmute(command),
    ));

    let argument: usize = command
        .takes_third_argument()
        .then_some(arguments.next_arg())
        .unwrap_or_default();
    fcntl_dispatch(file_descriptor, command, argument)
}
