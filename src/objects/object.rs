use std::{
    cmp::{max, min},
    ffi::c_void,
    fs::File,
    io::Read,
    marker::PhantomData,
    os::{fd::AsRawFd, unix::fs::FileExt},
    ptr::null_mut,
    slice,
};

use crate::{
    elf::{
        header::ElfHeader,
        program_header::{ProgramHeader, PT_LOAD},
    },
    libc::mem::{mmap, MapFlags, ProtectionFlags},
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

unsafe fn load_segment(
    in_memeory_base: *const c_void,
    file: &File,
    segment_program_header: &ProgramHeader,
) {
    debug_assert!(segment_program_header.p_type == PT_LOAD);

    let segment_start = page_size::get_page_start(
        in_memeory_base.byte_add(segment_program_header.p_vaddr) as usize,
    );

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
            in_memeory_base
                .byte_add(segment_program_header.p_vaddr)
                .byte_add(segment_program_header.p_filesz) as *mut u8,
            segment_program_header.p_memsz - segment_program_header.p_filesz as usize,
        )
        .fill(0);
    }
}

pub struct MapDependencies;
pub struct AllocateTLS;
pub struct GOTSetup;
pub struct Object<T> {
    object_data: ObjectData<Dynamic>,
    phantom_data: PhantomData<T>,
}

impl Object<MapDependencies> {
    pub unsafe fn map_from_file(mut file: File) -> Self {
        // Read the ELF header from file:
        let mut header_from_file: ElfHeader = unsafe { std::mem::zeroed() };
        let as_bytes = slice::from_raw_parts_mut(
            &mut header_from_file as *mut ElfHeader as *mut u8,
            size_of::<ElfHeader>(),
        );
        if let Err(error) = file.read_exact(as_bytes) {
            todo!();
        }

        // Read the program header table from file:
        let mut program_headers_from_file: Vec<ProgramHeader> =
            Vec::with_capacity(header_from_file.e_phnum as usize);
        let as_bytes = slice::from_raw_parts_mut(
            program_headers_from_file.as_mut_ptr() as *mut u8,
            size_of::<ProgramHeader>() * header_from_file.e_phnum as usize,
        );
        if let Err(error) = file.read_exact_at(as_bytes, header_from_file.e_phoff as u64) {
            todo!()
        }
        program_headers_from_file.set_len(header_from_file.e_phnum as usize);
        debug_assert!(program_headers_from_file
            .iter()
            .any(|h| h.p_type == PT_LOAD));

        // Reserve a continuous region of memory:
        let (min_addr, max_addr) = calculate_virtual_address_bounds(&program_headers_from_file);
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

        Self {
            object_data: ObjectData::from_base(base),
            phantom_data: PhantomData,
        }
    }

    pub fn for_each_dependency(self, f: impl FnMut(&str)) -> Object<AllocateTLS> {
        self.object_data.dependency_names().for_each(f);
        Object::<AllocateTLS> {
            phantom_data: PhantomData,
            ..self
        }
    }
}

impl Object<AllocateTLS> {
    pub fn allocate_tls(self) -> Object<GOTSetup> {
        todo!();
        Object::<GOTSetup> {
            phantom_data: PhantomData,
            ..self
        }
    }
}
