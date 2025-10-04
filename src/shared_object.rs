use core::ptr::null;
use core::{ffi::c_void, slice};
use std::alloc::{self};
use std::cmp::{max, min};
use std::fs::File;
use std::io::Read;
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::os::unix::fs::FileExt;
use std::ptr::{null_mut, slice_from_raw_parts_mut};

use crate::elf::dynamic_array::{DynamicArrayItem, DT_NEEDED, DT_NULL};
use crate::elf::program_header::PT_LOAD;
use crate::elf::relocate::RelocationSlices;
use crate::elf::symbol::SymbolTable;
use crate::page_size;
use crate::{
    elf::{
        dynamic_array::{
            DynamicArrayIter, DT_RELA, DT_RELAENT, DT_RELASZ, DT_STRTAB, DT_SYMENT, DT_SYMTAB,
        },
        header::{ElfHeader, ET_DYN},
        program_header::{ProgramHeader, PT_DYNAMIC, PT_PHDR, PT_TLS},
        relocate::Rela,
        string_table::StringTable,
        symbol::Symbol,
    },
    syscall::{exit, io, mmap},
    syscall_assert, syscall_debug_assert,
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
    (
        page_size::get_page_start(min_addr),
        page_size::get_page_end(max_addr),
    )
}

/// A struct repersenting a shared object in memory.
///
/// There are two ways to construct a `SharedObject`:
///
/// 1. From a slice of program headers:
///
/// 2. From a file descriptor:
pub struct SharedObject {
    pub base: *const (),
    pub relocations: RelocationSlices,
    pub needed_libraries: Vec<usize>, // Indexs into the string table...
    pub symbol_table: SymbolTable,
    pub string_table: StringTable,
    pub thread_local_block: Option<usize>,
}

impl SharedObject {
    pub unsafe fn from_headers(
        program_header_table: &[ProgramHeader],
        pseudorandom_bytes: *const [u8; 16],
    ) -> Self {
        let (mut base, mut dynamic_header, mut tls_program_header) = (null(), None, None);
        for header in program_header_table {
            match header.p_type {
                PT_PHDR => {
                    base = program_header_table.as_ptr().byte_sub(header.p_vaddr) as *const ();
                }
                PT_DYNAMIC => dynamic_header = Some(header),
                PT_TLS => tls_program_header = Some(header),
                _ => (),
            }
        }
        syscall_debug_assert!(dynamic_header.is_some());
        syscall_debug_assert!(tls_program_header.is_some());

        Self::build(base, dynamic_header.unwrap_unchecked(), Some(1))
    }

    pub unsafe fn from_file(mut file: File) -> Self {
        // ELf Header:
        let mut uninit_header: MaybeUninit<ElfHeader> = MaybeUninit::uninit();
        let as_bytes = slice::from_raw_parts_mut(
            uninit_header.as_mut_ptr() as *mut u8,
            size_of::<ElfHeader>(),
        );
        if let Err(error) = file.read_exact(as_bytes) {
            io::write(io::STD_ERR, "Error: could not read ElfHeader from file");
            exit::exit(1);
        }
        let header = uninit_header.assume_init();

        // Program Headers:
        let mut program_header_table: Vec<ProgramHeader> =
            Vec::with_capacity(header.e_phnum as usize);
        let as_bytes = slice::from_raw_parts_mut(
            program_header_table.as_mut_ptr() as *mut u8,
            header.e_phnum as usize * size_of::<ProgramHeader>(),
        );
        if let Err(error) = file.read_exact_at(as_bytes, header.e_phoff as u64) {
            io::write(
                io::STD_ERR,
                "Error: could not read &[ProgramHeader] from file",
            );
            exit::exit(1);
        }
        program_header_table.set_len(header.e_phnum as usize);
        syscall_debug_assert!(program_header_table.iter().any(|h| h.p_type == PT_LOAD));

        let (min_addr, max_addr) = calculate_virtual_address_bounds(&program_header_table);

        let base = mmap::mmap(
            null_mut(),
            max_addr - min_addr,
            mmap::PROT_EXEC | mmap::PROT_READ | mmap::PROT_WRITE,
            mmap::MAP_PRIVATE | mmap::MAP_ANONYMOUS,
            -1,
            0,
        ) as *const ();

        let (mut dynamic_header, mut tls_program_header) = (None, None);
        for header in &program_header_table {
            match header.p_type {
                PT_DYNAMIC => dynamic_header = Some(header),
                PT_TLS => tls_program_header = Some(header),
                PT_LOAD => {
                    let segment_start =
                        page_size::get_page_start(base.byte_add(header.p_vaddr) as usize);

                    let file_start = page_size::get_page_start(header.p_offset);
                    let file_length = (header.p_offset + header.p_filesz) - file_start;

                    const ELF_FLAG_EXEC: u32 = 0x1;
                    const ELF_FLAG_READ: u32 = 0x2;
                    const ELF_FLAG_WRITE: u32 = 0x4;

                    let flags = ((header.p_flags & ELF_FLAG_EXEC != 0) as usize * mmap::PROT_EXEC)
                        | ((header.p_flags & ELF_FLAG_READ != 0) as usize * mmap::PROT_READ)
                        | ((header.p_flags & ELF_FLAG_WRITE != 0) as usize * mmap::PROT_WRITE);

                    mmap::mmap(
                        segment_start as *mut u8,
                        file_length,
                        flags,
                        mmap::MAP_PRIVATE | mmap::MAP_FIXED,
                        file.as_raw_fd() as isize,
                        file_start,
                    );

                    if header.p_memsz > header.p_filesz {
                        slice::from_raw_parts_mut(
                            base.byte_add(header.p_vaddr).byte_add(header.p_filesz) as *mut u8,
                            header.p_memsz - header.p_filesz as usize,
                        )
                        .fill(0);
                    }
                }
                _ => (),
            }
        }

        syscall_debug_assert!(header == *(base as *const ElfHeader));

        let in_memory_program_header_table = slice::from_raw_parts(
            base.byte_add(header.e_phoff) as *const ProgramHeader,
            header.e_phnum as usize,
        );
        syscall_debug_assert!(in_memory_program_header_table == program_header_table.as_slice());

        let thread_local_block = tls_program_header.map(|header| 1); // TODO: tls

        syscall_debug_assert!(dynamic_header.is_some());
        Self::build(base, dynamic_header.unwrap(), thread_local_block)
    }

    unsafe fn build(
        base: *const (),
        dynamic_header: &ProgramHeader,
        thread_local_block: Option<usize>,
    ) -> Self {
        // Dynamic Arrary:
        let dynamic_array =
            DynamicArrayIter::new(base.byte_add(dynamic_header.p_vaddr) as *const DynamicArrayItem);
        syscall_debug_assert!(dynamic_array.clone().count() != 0);

        let mut rela_pointer: *const Rela = null();
        let mut rela_count = 0;

        let mut symbol_table_pointer: *const Symbol = null();
        let mut string_table_pointer: *const u8 = null();
        let mut needed_libraries = Vec::new();
        for item in dynamic_array {
            match item.d_tag {
                DT_NEEDED => needed_libraries.push(item.d_un.d_val),
                DT_RELA => {
                    rela_pointer = base.byte_add(item.d_un.d_ptr.addr()) as *const Rela;
                }
                DT_RELASZ => {
                    rela_count = item.d_un.d_val / core::mem::size_of::<Rela>();
                }
                #[cfg(debug_assertions)]
                DT_RELAENT => {
                    syscall_assert!(item.d_un.d_val as usize == size_of::<Rela>())
                }
                // Tables:
                DT_SYMTAB => {
                    symbol_table_pointer = base.byte_add(item.d_un.d_ptr.addr()) as *const Symbol
                }
                DT_STRTAB => {
                    string_table_pointer = base.byte_add(item.d_un.d_ptr.addr()) as *const u8
                }
                #[cfg(debug_assertions)]
                DT_SYMENT => {
                    syscall_assert!(item.d_un.d_val as usize == size_of::<Symbol>())
                }
                _ => (),
            }
        }

        syscall_debug_assert!(rela_pointer != null());
        let rela_slice = slice::from_raw_parts(rela_pointer, rela_count);

        Self {
            base,
            relocations: RelocationSlices { rela_slice },
            needed_libraries,
            symbol_table: SymbolTable::new(symbol_table_pointer),
            string_table: StringTable::new(string_table_pointer),
            thread_local_block,
        }
    }
}
