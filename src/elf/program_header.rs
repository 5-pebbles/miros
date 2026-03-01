use bitbybit::bitfield;

use crate::libc::mem::ProtectionFlags;

pub const PT_LOAD: u32 = 1;
pub const PT_DYNAMIC: u32 = 2;
pub const PT_PHDR: u32 = 6;
pub const PT_TLS: u32 = 7;

#[bitfield(u32)]
#[derive(PartialEq)]
pub struct ProgramHeaderFlags {
    #[bit(0, rw)]
    executable: bool,
    #[bit(1, rw)]
    readable: bool,
    #[bit(2, rw)]
    writable: bool,
}

impl ProgramHeaderFlags {
    pub fn into_protection_flags(self) -> ProtectionFlags {
        ProtectionFlags::ZERO
            .with_readable(self.readable())
            .with_writable(self.writable())
            .with_executable(self.executable())
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct ProgramHeader {
    pub p_type: u32,
    #[cfg(target_pointer_width = "64")]
    pub p_flags: ProgramHeaderFlags,
    pub p_offset: usize,
    pub p_vaddr: usize,
    pub p_paddr: usize,
    pub p_filesz: usize,
    pub p_memsz: usize,
    #[cfg(target_pointer_width = "32")]
    pub p_flags: ProgramHeaderFlags,
    pub p_align: usize,
}
