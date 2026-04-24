use std::os::fd::{AsRawFd, BorrowedFd};

use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc,
    syscall::{syscall, Syscall},
};

// TODO: structure fields with proper types (e.g. enums for st_mode, bitfields for permissions)
#[repr(C)]
pub struct FileStatus {
    pub device: u64,
    pub inode: u64,
    pub hard_link_count: u64,
    pub mode: u32,
    pub user_id: u32,
    pub group_id: u32,
    _pad0: u32,
    pub device_type: u64,
    pub size_in_bytes: i64,
    pub block_size: i64,
    pub block_count: i64,
    pub access_time_seconds: u64,
    pub access_time_nanoseconds: u64,
    pub modification_time_seconds: u64,
    pub modification_time_nanoseconds: u64,
    pub change_time_seconds: u64,
    pub change_time_nanoseconds: u64,
    _reserved: [u64; 3],
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn fstat64(
    file_descriptor: BorrowedFd<'_>,
    file_status_pointer: *mut FileStatus,
) -> i32 {
    signature_matches_libc!(libc::fstat64(
        std::mem::transmute(file_descriptor),
        std::mem::transmute(file_status_pointer),
    ));

    let result = syscall!(
        Syscall::FStat,
        file_descriptor.as_raw_fd(),
        file_status_pointer
    );

    if result < 0 {
        set_errno(Errno(result.abs() as u32));
        -1
    } else {
        result as i32
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn stat64(pathname: *const i8, file_status_pointer: *mut FileStatus) -> i32 {
    signature_matches_libc!(libc::stat64(
        std::mem::transmute(pathname),
        std::mem::transmute(file_status_pointer),
    ));

    let result = syscall!(Syscall::Stat, pathname, file_status_pointer);

    if result < 0 {
        set_errno(Errno(result.abs() as u32));
        -1
    } else {
        result as i32
    }
}
