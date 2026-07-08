use std::ffi::c_void;

use crate::elf::program_header::ProgramHeader;

#[derive(Clone, Copy)]
pub struct TlsTemplate {
    pub template_pointer: *const u8,
    pub template_size: usize,
    pub block_size: usize,
    pub alignment: usize,
}

impl TlsTemplate {
    /// The template is read from the mapped image: `p_vaddr`, not `p_offset` — the RW segment's file offset and vaddr diverge.
    pub unsafe fn from_program_header(base: *const c_void, tls_header: &ProgramHeader) -> Self {
        Self {
            template_pointer: base.byte_add(tls_header.p_vaddr) as *const u8,
            template_size: tls_header.p_filesz,
            block_size: tls_header.p_memsz,
            alignment: tls_header.p_align,
        }
    }
}
