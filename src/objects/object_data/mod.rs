mod dynamic_trait_objects;

pub use dynamic_trait_objects::{AnyDynamic, Dynamic, DynamicObject, NonDynamic};

use std::slice;
use std::{ffi::c_void, ptr::null};

use crate::elf::dynamic_array::{DT_NEEDED, DT_PLTGOT};
use crate::elf::header::ElfHeader;
use crate::elf::program_header::{PT_DYNAMIC, PT_PHDR, PT_TLS};
use crate::start::auxiliary_vector::AuxiliaryVectorItem;
#[cfg(debug_assertions)]
use crate::{
    elf::dynamic_array::{DT_RELAENT, DT_SYMENT},
    io_macros::syscall_assert,
};
use crate::{
    elf::{
        dynamic_array::{
            DynamicArrayItem, DynamicArrayIter, DT_INIT_ARRAY, DT_INIT_ARRAYSZ, DT_RELA, DT_RELASZ,
            DT_STRTAB, DT_SYMTAB,
        },
        program_header::ProgramHeader,
        relocate::Rela,
        string_table::StringTable,
        symbol::{Symbol, SymbolTable},
    },
    io_macros::syscall_debug_assert,
};

pub type InitArrayFunction =
    extern "C" fn(usize, *const *const u8, *const *const u8, *const AuxiliaryVectorItem);

pub struct ThreadLocalAllocation {
    block_id: usize,
    block_offset: usize,
    // This enables lazy per-thread allocation when thread.generation < self.generation & modules are dlopen'd after threads are created
    generation: usize,
}

impl ThreadLocalAllocation {
    pub fn new(block_id: usize, block_offset: usize) -> Self {
        // TODO: This should probably get and increment the global generation counter... someday, but not today.
        Self {
            block_id,
            block_offset,
            generation: 0,
        }
    }
}

pub struct ThreadLocalData {
    pub tls_program_header: ProgramHeader,
    pub thread_local_allocation: Option<ThreadLocalAllocation>,
}

pub struct ObjectData<T: AnyDynamic> {
    pub base: *const c_void,
    pub dynamic_array: *const DynamicArrayItem,
    pub global_offset_table: *const usize,
    pub string_table: StringTable,
    pub symbol_table: SymbolTable,
    pub rela_slice: &'static [Rela],
    pub tls_data: Option<ThreadLocalData>,
    pub init_array: Option<&'static [InitArrayFunction]>,

    pub needed_libraries: T,
}

impl ObjectData<Dynamic> {
    pub fn dependency_names(&self) -> impl Iterator<Item = &str> {
        self.needed_libraries
            .iter()
            .map(|needed_library_index| unsafe { self.string_table.get(*needed_library_index) })
    }
}

impl<T: AnyDynamic> ObjectData<T> {
    pub unsafe fn from_base(base: *const c_void) -> Self {
        // ELf Header:
        let header = &*(base as *const ElfHeader);
        syscall_debug_assert!(header.e_phentsize == size_of::<ProgramHeader>() as u16);

        // Program Headers:
        let program_header_table = slice::from_raw_parts(
            base.byte_add(header.e_phoff) as *const ProgramHeader,
            header.e_phnum as usize,
        );

        let mut dynamic_program_header = null();
        let mut tls_program_header = None;
        for header in program_header_table {
            match header.p_type {
                PT_DYNAMIC => dynamic_program_header = header,
                PT_TLS => tls_program_header = Some(header.to_owned()),
                _ => (),
            }
        }

        Self::build_internal(base, dynamic_program_header, tls_program_header)
    }

    pub unsafe fn from_program_headers(
        program_header_table: &'static [ProgramHeader],
    ) -> ObjectData<T> {
        let (mut base, mut dynamic_program_header) = (null(), null());
        let mut tls_program_header = None;
        for header in program_header_table {
            match header.p_type {
                PT_PHDR => {
                    base = program_header_table.as_ptr().byte_sub(header.p_vaddr) as *const c_void;
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
    ) -> Self {
        syscall_debug_assert!(dynamic_program_header != null());

        // Dynamic Arrary:
        let dynamic_array =
            base.byte_add((*dynamic_program_header).p_vaddr) as *const DynamicArrayItem;

        let mut global_offset_table_pointer: *const usize = null();
        let mut string_table_pointer: *const u8 = null();
        let mut symbol_table_pointer: *const Symbol = null();

        let mut rela_pointer: *const Rela = null();
        let mut rela_count = 0;

        let mut init_array_pointer: *const InitArrayFunction = null();
        let mut init_array_size = 0;

        let mut needed_libraries = T::default();
        for item in DynamicArrayIter::new(dynamic_array) {
            match item.d_tag {
                DT_PLTGOT => {
                    global_offset_table_pointer =
                        base.byte_add(item.d_un.d_ptr.addr()) as *const usize
                }
                DT_STRTAB => {
                    string_table_pointer = base.byte_add(item.d_un.d_ptr.addr()) as *const u8
                }
                DT_SYMTAB => {
                    symbol_table_pointer = base.byte_add(item.d_un.d_ptr.addr()) as *const Symbol
                }
                #[cfg(debug_assertions)]
                DT_SYMENT => syscall_assert!(item.d_un.d_val == size_of::<Symbol>()),

                DT_RELA => {
                    rela_pointer = base.byte_add(item.d_un.d_ptr.addr()) as *const Rela;
                }
                DT_RELASZ => {
                    rela_count = item.d_un.d_val / core::mem::size_of::<Rela>();
                }
                #[cfg(debug_assertions)]
                DT_RELAENT => {
                    syscall_assert!(item.d_un.d_val == size_of::<Rela>())
                }

                DT_INIT_ARRAY => {
                    init_array_pointer =
                        base.byte_add(item.d_un.d_ptr.addr()) as *const InitArrayFunction;
                }
                DT_INIT_ARRAYSZ => {
                    init_array_size = item.d_un.d_val / size_of::<usize>();
                }

                DT_NEEDED => {
                    needed_libraries.handle_needed(item.d_un);
                }
                _ => (),
            }
        }

        let string_table = StringTable::new(string_table_pointer);
        let symbol_table = SymbolTable::new(symbol_table_pointer);

        syscall_debug_assert!(rela_pointer != null());
        let rela_slice = slice::from_raw_parts(rela_pointer, rela_count);

        let init_array = if init_array_pointer.is_null() || init_array_size == 0 {
            None
        } else {
            Some(slice::from_raw_parts(init_array_pointer, init_array_size))
        };

        Self {
            base,
            dynamic_array,
            global_offset_table: global_offset_table_pointer,
            string_table,
            symbol_table,
            rela_slice,
            tls_data: tls_program_header.map(|tls_program_header| ThreadLocalData {
                tls_program_header,
                thread_local_allocation: None,
            }),
            init_array,
            needed_libraries,
        }
    }
}
