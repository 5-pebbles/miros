pub mod dynamic_fields;
mod dynamic_trait_objects;
mod hash_tables;
mod path_resolver;
mod thread_local;

use core::slice;
use std::{
    cmp::{max, min},
    ffi::c_void,
    fs::File,
    io::Read,
    os::{fd::AsRawFd, unix::fs::FileExt},
    ptr::{self, null, null_mut},
};

pub use dynamic_fields::DynamicFields;
pub use dynamic_trait_objects::{AnyDynamic, Dynamic, NonDynamic};
pub use thread_local::{ThreadLocalAllocation, ThreadLocalData};

use crate::{
    elf::{
        dynamic_array::DynamicArrayItem,
        header::ElfHeader,
        program_header::{ProgramHeader, PT_DYNAMIC, PT_LOAD, PT_PHDR, PT_TLS},
    },
    error::MirosError,
    io_macros::syscall_debug_assert,
    libc::mem::{mmap, MapFlags, ProtectionFlags},
    page_size,
    start::auxiliary_vector::AuxiliaryVectorItem,
};

pub type InitArrayFunction =
    extern "C" fn(usize, *const *const u8, *const *const u8, *const AuxiliaryVectorItem);

pub struct ObjectData<T: AnyDynamic> {
    pub base: *const c_void,
    pub dynamic_fields: DynamicFields<T>,
    pub tls_data: Option<ThreadLocalData>,
}

impl<T: AnyDynamic> ObjectData<T> {
    pub unsafe fn from_base(base: *const c_void) -> Result<Self, MirosError> {
        // ELf Header:
        let header = &*(base as *const ElfHeader);
        syscall_debug_assert!(header.e_phentsize == size_of::<ProgramHeader>() as u16);

        // Program Headers:
        let program_header_table = ptr::slice_from_raw_parts(
            base.byte_add(header.e_phoff) as *const ProgramHeader,
            header.e_phnum as usize,
        );

        let mut dynamic_program_header = null();
        let mut tls_program_header = None;
        for header in &*program_header_table {
            match header.p_type {
                PT_DYNAMIC => dynamic_program_header = header,
                PT_TLS => tls_program_header = Some(header.to_owned()),
                _ => (),
            }
        }

        Self::build_internal(base, dynamic_program_header, tls_program_header)
    }

    pub unsafe fn from_program_headers(
        program_header_table: *const [ProgramHeader],
    ) -> Result<Self, MirosError> {
        let (mut base, mut dynamic_program_header) = (null(), null());
        let mut tls_program_header = None;
        for header in &*program_header_table {
            match header.p_type {
                PT_PHDR => {
                    base =
                        (*program_header_table).as_ptr().byte_sub(header.p_vaddr) as *const c_void;
                }
                PT_DYNAMIC => dynamic_program_header = header,
                PT_TLS => tls_program_header = Some(header.to_owned()),
                _ => (),
            }
        }

        Self::build_internal(base, dynamic_program_header, tls_program_header)
    }

    unsafe fn build_internal(
        base: *const c_void,
        dynamic_program_header: *const ProgramHeader,
        tls_program_header: Option<ProgramHeader>,
    ) -> Result<Self, MirosError> {
        syscall_debug_assert!(dynamic_program_header != null());

        let dynamic_array =
            base.byte_add((*dynamic_program_header).p_vaddr) as *const DynamicArrayItem;

        Ok(Self {
            base,
            dynamic_fields: DynamicFields::from_dynamic_array(base, dynamic_array)?,
            tls_data: tls_program_header.map(|tls_program_header| ThreadLocalData {
                tls_program_header,
                thread_local_allocation: None,
            }),
        })
    }
}

impl ObjectData<Dynamic> {
    pub unsafe fn from_file(mut file: File) -> Result<Self, MirosError> {
        // Read the ELF header from file:
        let mut header_from_file: ElfHeader = unsafe { std::mem::zeroed() };
        let as_bytes = slice::from_raw_parts_mut(
            &mut header_from_file as *mut ElfHeader as *mut u8,
            size_of::<ElfHeader>(),
        );
        file.read_exact(as_bytes)
            .map_err(|_| MirosError::ElfReadError("failed to read ELF header".to_string()))?;

        // Read the program header table from file:
        let mut program_headers_from_file: Vec<ProgramHeader> =
            Vec::with_capacity(header_from_file.e_phnum as usize);
        let as_bytes = slice::from_raw_parts_mut(
            program_headers_from_file.as_mut_ptr() as *mut u8,
            size_of::<ProgramHeader>() * header_from_file.e_phnum as usize,
        );
        file.read_exact_at(as_bytes, header_from_file.e_phoff as u64)
            .map_err(|_| {
                MirosError::ElfReadError("failed to read program header table".to_string())
            })?;
        program_headers_from_file.set_len(header_from_file.e_phnum as usize);
        debug_assert!(program_headers_from_file
            .iter()
            .any(|header| header.p_type == PT_LOAD));

        // Reserve a continuous region of memory:
        let (min_addr, max_addr) =
            calculate_virtual_address_bounds(&program_headers_from_file);
        let protection_flags = ProtectionFlags::ZERO
            .with_executable(true)
            .with_readable(true)
            .with_writable(true);
        let map_flags = MapFlags::ZERO.with_private(true).with_anonymous(true);
        let base = mmap(
            null_mut(),
            max_addr - min_addr,
            protection_flags,
            map_flags,
            -1,
            0,
        ) as *const c_void;

        // Load all segments:
        program_headers_from_file
            .iter()
            .filter(|program_header| program_header.p_type == PT_LOAD)
            .for_each(|program_header| load_segment(base, &file, program_header));

        Self::from_base(base)
    }
}

fn calculate_virtual_address_bounds(program_header_table: &[ProgramHeader]) -> (usize, usize) {
    let mut min_addr = usize::MAX;
    let mut max_addr = 0;

    for header in program_header_table {
        if header.p_type != PT_LOAD {
            continue;
        }

        let start = header.p_vaddr as usize;
        let end = start + header.p_memsz as usize;

        min_addr = min(min_addr, start);
        max_addr = max(max_addr, end);
    }

    // Align bounds to page boundaries
    unsafe {
        (
            page_size::get_page_start(min_addr),
            page_size::get_page_end(max_addr),
        )
    }
}

unsafe fn load_segment(
    in_memory_base: *const c_void,
    file: &File,
    segment_program_header: &ProgramHeader,
) {
    debug_assert!(segment_program_header.p_type == PT_LOAD);

    let segment_start =
        page_size::get_page_start(in_memory_base.byte_add(segment_program_header.p_vaddr) as usize);

    let file_start = page_size::get_page_start(segment_program_header.p_offset);
    let file_length =
        (segment_program_header.p_offset + segment_program_header.p_filesz) - file_start;

    let protection_flags = segment_program_header.p_flags.into_protection_flags();
    let map_flags = MapFlags::ZERO.with_private(true).with_fixed(true);

    mmap(
        segment_start as *mut u8,
        file_length,
        protection_flags,
        map_flags,
        file.as_raw_fd(),
        file_start,
    );

    if segment_program_header.p_memsz > segment_program_header.p_filesz {
        slice::from_raw_parts_mut(
            in_memory_base
                .byte_add(segment_program_header.p_vaddr)
                .byte_add(segment_program_header.p_filesz) as *mut u8,
            segment_program_header.p_memsz - segment_program_header.p_filesz as usize,
        )
        .fill(0);
    }
}
