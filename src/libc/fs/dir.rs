use std::{
    alloc::Layout,
    ffi::c_char,
    mem::{self, MaybeUninit},
    os::fd::RawFd,
    ptr, slice,
};

use super::open::{O_CLOEXEC, O_DIRECTORY};
use crate::{
    libc::errno::{set_errno, Errno},
    signature_matches_libc, syscall,
    syscall::Syscall,
};

const AT_FDCWD: isize = -100;

/// Byte offset of the name field in a kernel `getdents64` record.
const RECORD_HEADER_SIZE: usize = mem::offset_of!(LinuxDirent64, file_type) + 1;

const BUFFER_CAPACITY: usize = 32 * 1024;

/// The kernel's `getdents64` record header; the NUL-terminated name follows inline, bounded by `record_length`.
#[repr(C)]
struct LinuxDirent64 {
    inode: u64,
    offset: i64,
    record_length: u16,
    file_type: u8,
}

/// glibc-ABI `struct dirent64`; `dirent` has the identical layout on x86_64.
#[repr(C)]
pub struct DirectoryEntry {
    pub inode: u64,
    pub offset: i64,
    pub record_length: u16,
    pub file_type: u8,
    pub name: [c_char; 256],
}

/// glibc's `DIR`: an open directory descriptor plus the `getdents64` scratch buffer.
pub struct DirectoryStream {
    file_descriptor: RawFd,
    /// Valid bytes in `buffer`, as returned by the last `getdents64`.
    buffer_length: usize,
    /// Bytes of `buffer` already consumed by `readdir`.
    buffer_offset: usize,
    /// Scratch storage for the `readdir` return value; invalidated by the next call.
    entry: DirectoryEntry,
    buffer: MaybeUninit<[u8; BUFFER_CAPACITY]>,
}

impl DirectoryStream {
    unsafe fn next_entry(&mut self) -> *mut DirectoryEntry {
        if self.buffer_offset == self.buffer_length {
            let result = syscall!(
                Syscall::GetDents64,
                self.file_descriptor,
                self.buffer.as_mut_ptr(),
                BUFFER_CAPACITY
            );
            if result < 0 {
                set_errno(Errno(result.abs() as u32));
                return ptr::null_mut();
            }
            // End of stream: return null without touching errno.
            if result == 0 {
                return ptr::null_mut();
            }
            self.buffer_length = result as usize;
            self.buffer_offset = 0;
        }

        let record_pointer = self
            .buffer
            .as_ptr()
            .cast::<u8>()
            .add(self.buffer_offset)
            .cast::<LinuxDirent64>();
        let record = record_pointer.read_unaligned();
        self.buffer_offset += record.record_length as usize;

        let name_bytes = slice::from_raw_parts(
            record_pointer.cast::<u8>().add(RECORD_HEADER_SIZE),
            record.record_length as usize - RECORD_HEADER_SIZE,
        );
        // The kernel NUL-terminates the name within its record.
        let name_length = name_bytes
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(name_bytes.len());

        self.entry.inode = record.inode;
        self.entry.offset = record.offset;
        self.entry.file_type = record.file_type;
        self.entry.record_length = (mem::offset_of!(DirectoryEntry, name) + name_length + 1) as u16;
        // The kernel caps names at NAME_MAX (255), so `name` always fits.
        ptr::copy_nonoverlapping(
            name_bytes.as_ptr().cast::<c_char>(),
            self.entry.name.as_mut_ptr(),
            name_length,
        );
        *self.entry.name.get_mut(name_length).unwrap() = 0;

        &mut self.entry
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn opendir(pathname: *const c_char) -> *mut DirectoryStream {
    signature_matches_libc!(std::mem::transmute(libc::opendir(pathname)));

    let result = syscall!(
        Syscall::OpenAt,
        AT_FDCWD,
        pathname,
        O_DIRECTORY | O_CLOEXEC,
        0
    );
    if result < 0 {
        set_errno(Errno(result.abs() as u32));
        return ptr::null_mut();
    }
    let file_descriptor = result as RawFd;

    let layout = Layout::new::<DirectoryStream>();
    let stream = std::alloc::alloc(layout).cast::<DirectoryStream>();
    if stream.is_null() {
        syscall!(Syscall::Close, file_descriptor);
        set_errno(Errno::NOMEM);
        return ptr::null_mut();
    }

    stream.write(DirectoryStream {
        file_descriptor,
        buffer_length: 0,
        buffer_offset: 0,
        entry: mem::zeroed(),
        buffer: MaybeUninit::uninit(),
    });
    stream
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn readdir(stream: *mut DirectoryStream) -> *mut DirectoryEntry {
    signature_matches_libc!(std::mem::transmute(libc::readdir(std::mem::transmute(
        stream
    ))));

    (*stream).next_entry()
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn readdir64(stream: *mut DirectoryStream) -> *mut DirectoryEntry {
    signature_matches_libc!(std::mem::transmute(libc::readdir64(std::mem::transmute(
        stream
    ))));

    (*stream).next_entry()
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn closedir(stream: *mut DirectoryStream) -> i32 {
    signature_matches_libc!(libc::closedir(std::mem::transmute(stream)));

    let stream = &mut *stream;
    let result = syscall!(Syscall::Close, stream.file_descriptor);
    std::alloc::dealloc(
        (stream as *mut DirectoryStream).cast::<u8>(),
        Layout::new::<DirectoryStream>(),
    );

    if result < 0 {
        set_errno(Errno(result.abs() as u32));
        -1
    } else {
        0
    }
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn dirfd(stream: *const DirectoryStream) -> i32 {
    signature_matches_libc!(libc::dirfd(std::mem::transmute(stream)));

    (*stream).file_descriptor
}

#[cfg(test)]
mod tests {
    use std::{ffi::CStr, fs};

    use super::*;
    use crate::libc::errno::errno;

    fn create_test_directory(test_name: &str) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("miros_{}_{}", test_name, std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("alpha.txt"), b"a").unwrap();
        fs::create_dir(directory.join("subdir")).unwrap();
        directory
    }

    #[test]
    fn opendir_readdir_dirfd_closedir_roundtrip() {
        let directory = create_test_directory("dirent_roundtrip");
        let path = std::ffi::CString::new(directory.to_str().unwrap()).unwrap();

        let stream = unsafe { opendir(path.as_ptr()) };
        assert!(!stream.is_null());
        assert!(unsafe { dirfd(stream) } >= 0);

        let mut names: Vec<String> = Vec::new();
        loop {
            let entry = unsafe { readdir(stream) };
            if entry.is_null() {
                break;
            }
            let entry = unsafe { &*entry };
            let name = unsafe { CStr::from_ptr(entry.name.as_ptr()) };
            assert!(
                entry.record_length as usize
                    >= mem::offset_of!(DirectoryEntry, name) + name.count_bytes() + 1
            );
            names.push(name.to_str().unwrap().to_owned());
        }

        for expected in [".", "..", "alpha.txt", "subdir"] {
            assert!(
                names.iter().any(|name| name == expected),
                "missing {expected}"
            );
        }
        // End of stream is stable and leaves errno untouched.
        set_errno(Errno(42));
        assert!(unsafe { readdir(stream) }.is_null());
        assert_eq!(errno.get(), Errno(42));

        assert_eq!(unsafe { closedir(stream) }, 0);
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn readdir_refills_the_buffer_across_getdents_calls() {
        // 2048 files with short names overflow the 32 KiB buffer, forcing refills.
        let directory =
            std::env::temp_dir().join(format!("miros_dirent_refill_{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir(&directory).unwrap();
        for index in 0..2048 {
            fs::write(directory.join(format!("f{index}")), b"").unwrap();
        }
        let path = std::ffi::CString::new(directory.to_str().unwrap()).unwrap();

        let stream = unsafe { opendir(path.as_ptr()) };
        assert!(!stream.is_null());

        let mut count = 0;
        while !unsafe { readdir(stream) }.is_null() {
            count += 1;
        }
        // 2048 files plus "." and "..".
        assert_eq!(count, 2050);

        assert_eq!(unsafe { closedir(stream) }, 0);
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn opendir_fails_for_a_missing_directory() {
        let stream = unsafe { opendir(c"/nonexistent/miros_dirent".as_ptr()) };
        assert!(stream.is_null());
        assert_eq!(errno.get(), Errno(linux_raw_sys::errno::ENOENT));
    }

    #[test]
    fn opendir_fails_for_a_regular_file() {
        let file = std::env::temp_dir().join(format!("miros_dirent_file_{}", std::process::id()));
        fs::write(&file, b"a").unwrap();
        let path = std::ffi::CString::new(file.to_str().unwrap()).unwrap();

        let stream = unsafe { opendir(path.as_ptr()) };
        assert!(stream.is_null());
        assert_eq!(errno.get(), Errno(linux_raw_sys::errno::ENOTDIR));

        fs::remove_file(&file).unwrap();
    }
}
