use std::{ffi::c_void, marker::PhantomData, ptr::null, slice};

#[cfg(debug_assertions)]
use crate::elf::dynamic_array::{DT_RELAENT, DT_SYMENT};
use crate::{
    elf::{
        dynamic_array::{
            DynamicArrayItem, DynamicArrayIter, DT_NEEDED, DT_RELA, DT_RELASZ, DT_STRTAB, DT_SYMTAB,
        },
        header::{ElfHeader, ET_DYN},
        program_header::{ProgramHeader, PT_DYNAMIC, PT_PHDR, PT_TLS},
        relocate::Rela,
        string_table::StringTable,
        symbol::{Symbol, SymbolTable},
    },
    io_macros::{syscall_assert, syscall_debug_assert},
    libc::environ::set_environ_pointer,
    objects::relocate::RelaRelocatable,
};

pub struct EarlyRelocate;
pub struct Dependencies;
pub struct Relocate;
pub struct AllocateTLS;
pub struct InitArray;

pub struct Miros<T> {
    base_address: *const c_void,
    dynamic_array: *const DynamicArrayItem,
    string_table: StringTable,
    symbol_table: SymbolTable,
    rela_slice: &'static [Rela],
    tls_program_header: *const ProgramHeader,
    phantom_data: PhantomData<T>,
}

impl Miros<EarlyRelocate> {
    #[inline(always)]
    pub unsafe fn from_base(base: *const c_void) -> Miros<EarlyRelocate> {
        // ELf Header:
        let header = &*(base as *const ElfHeader);
        syscall_debug_assert!(header.e_type == ET_DYN);
        syscall_debug_assert!(header.e_phentsize == size_of::<ProgramHeader>() as u16);

        // Program Headers:
        let program_header_table = slice::from_raw_parts(
            base.byte_add(header.e_phoff) as *const ProgramHeader,
            header.e_phnum as usize,
        );

        let (mut dynamic_program_header, mut tls_program_header) = (null(), null());
        for header in program_header_table {
            match header.p_type {
                PT_DYNAMIC => dynamic_program_header = header,
                PT_TLS => tls_program_header = header,
                _ => (),
            }
        }

        Self::build(base, dynamic_program_header, tls_program_header)
    }

    #[inline(always)]
    pub unsafe fn from_program_headers(
        program_header_table: &'static [ProgramHeader],
    ) -> Miros<EarlyRelocate> {
        let (mut base, mut dynamic_program_header, mut tls_program_header) =
            (null(), null(), null());
        for header in program_header_table {
            match header.p_type {
                PT_PHDR => {
                    base = program_header_table.as_ptr().byte_sub(header.p_vaddr) as *const c_void;
                }
                PT_DYNAMIC => dynamic_program_header = header,
                PT_TLS => tls_program_header = header,
                _ => (),
            }
        }

        Self::build(base, dynamic_program_header, tls_program_header)
    }

    #[inline(always)]
    #[must_use]
    unsafe fn build(
        base: *const c_void,
        dynamic_program_header: *const ProgramHeader,
        tls_program_header: *const ProgramHeader,
    ) -> Miros<EarlyRelocate> {
        syscall_debug_assert!(dynamic_program_header != null());

        // Dynamic Arrary:
        let dynamic_array =
            base.byte_add((*dynamic_program_header).p_vaddr) as *const DynamicArrayItem;

        let mut string_table_pointer: *const u8 = null();
        let mut symbol_table_pointer: *const Symbol = null();

        let mut rela_pointer: *const Rela = null();
        let mut rela_count = 0;
        for item in DynamicArrayIter::new(dynamic_array) {
            match item.d_tag {
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
                DT_STRTAB => {
                    string_table_pointer = base.byte_add(item.d_un.d_ptr.addr()) as *const u8
                }
                DT_SYMTAB => {
                    symbol_table_pointer = base.byte_add(item.d_un.d_ptr.addr()) as *const Symbol
                }
                #[cfg(debug_assertions)]
                DT_SYMENT => syscall_assert!(item.d_un.d_val == size_of::<Symbol>()),
                _ => (),
            }
        }

        let string_table = StringTable::new(string_table_pointer);
        let symbol_table = SymbolTable::new(symbol_table_pointer);

        syscall_debug_assert!(rela_pointer != null());
        let rela_slice = slice::from_raw_parts(rela_pointer, rela_count);

        Miros::<EarlyRelocate> {
            base_address: base,
            dynamic_array,
            string_table,
            symbol_table,
            rela_slice,
            tls_program_header,
            phantom_data: PhantomData,
        }
    }
}

impl RelaRelocatable for Miros<EarlyRelocate> {
    fn base(&self) -> Result<*const c_void, Self::RelaError> {
        Ok(self.base_address)
    }
}

impl Miros<EarlyRelocate> {
    pub fn early_relocate(self) -> Miros<Dependencies> {
        unsafe {
            use crate::elf::relocate::{R_X86_64_IRELATIVE, R_X86_64_RELATIVE};
            self.rela_relocate(
                self.rela_slice
                    .into_iter()
                    .filter(|rela| matches!(rela.r_type(), R_X86_64_RELATIVE | R_X86_64_IRELATIVE)),
            )
            .unwrap();
        }

        Miros::<Dependencies> {
            phantom_data: PhantomData,
            ..self
        }
    }
}

impl Miros<Dependencies> {
    pub unsafe fn load_dependencies(self, environ_pointer: *mut *mut u8) -> Miros<Relocate> {
        unsafe { set_environ_pointer(environ_pointer) };

        let mut needed_libraries: Vec<usize> = Vec::new(); // Indexs into the string table...
        for item in DynamicArrayIter::new(self.dynamic_array) {
            match item.d_tag {
                DT_NEEDED => needed_libraries.push(unsafe { item.d_un.d_val }),
                _ => (),
            }
        }

        for library in needed_libraries {
            unsafe { self.string_table.get(library) };
        }

        Miros::<Relocate> {
            phantom_data: PhantomData,
            ..self
        }
    }
}
