use std::ffi::c_void;
use std::os::unix::fs::FileExt;
use std::ptr::null_mut;
use std::slice;
use std::{
    cmp::{max, min},
    fs::File,
    io::Read,
    marker::PhantomData,
};

use crate::elf::program_header::{PT_DYNAMIC, PT_TLS};
use crate::io_macros::syscall_debug_assert;
use crate::libc::mem::{mmap, MapFlags, ProtectionFlags};
use crate::{
    elf::{
        header::ElfHeader,
        program_header::{ProgramHeader, PT_LOAD},
    },
    objects::object_data::{Dynamic, ObjectData},
    page_size,
};

fn calculate_virtual_address_bounds(program_header_table: &[ProgramHeader]) -> (usize, usize) {
    let mut min_addr = usize::MAX;
    let mut max_addr = 0;

    for header in program_header_table {
        // Skip non-loadable segments
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

fn load_segment(
    in_memeory_base: *mut c_void,
    file_header: &ElfHeader,
    segment_program_header: &ProgramHeader,
) {
}

pub struct LoadDependencies;

pub struct Dependency<T> {
    object_base: ObjectData<Dynamic>,
    phantom_data: PhantomData<T>,
}

impl Dependency<LoadDependencies> {
    pub unsafe fn from_file(mut file: File) -> Self {
        // Read the ELF header from file:
        let mut header: ElfHeader = unsafe { std::mem::zeroed() };
        let as_bytes = slice::from_raw_parts_mut(
            &mut header as *mut ElfHeader as *mut u8,
            size_of::<ElfHeader>(),
        );
        if let Err(error) = file.read_exact(as_bytes) {
            todo!();
        }

        // Read the program header table from file:
        let mut program_headers: Vec<ProgramHeader> = Vec::with_capacity(header.e_phnum as usize);
        let as_bytes = slice::from_raw_parts_mut(
            program_headers.as_mut_ptr() as *mut u8,
            size_of::<ProgramHeader>() * header.e_phnum as usize,
        );
        if let Err(error) = file.read_exact_at(as_bytes, header.e_phoff as u64) {
            todo!()
        }
        program_headers.set_len(header.e_phnum as usize);
        syscall_debug_assert!(program_headers.iter().any(|h| h.p_type == PT_LOAD));

        // Reserve a continuous region of memory:
        let (min_addr, max_addr) = calculate_virtual_address_bounds(&program_headers);
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

        // Parse program header table:
        let (mut dynamic_program_header, mut tls_program_header) = (null(), null());
        for program_header in program_headers {
            match program_header.p_type {
                PT_DYNAMIC => dynamic_program_header = header,
                PT_TLS => tls_program_header = header,
                _ => (),
            }
        }
        todo!();
    }
}
