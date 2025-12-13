use std::ops::{Deref, DerefMut};
use std::slice;
use std::{ffi::c_void, ptr::null};

use crate::elf::dynamic_array::{DynamicArrayUnion, DT_NEEDED};
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
    objects::InitArrayFunction,
};

mod private {
    use super::{Dynamic, NonDynamic};

    pub trait Sealed {}
    impl Sealed for NonDynamic {}
    impl Sealed for Dynamic {}
}

pub trait DynamicObject: private::Sealed {
    fn handle_needed(&mut self, dynamic_item: DynamicArrayUnion);
}

#[derive(Default)]
pub struct NonDynamic;

impl DynamicObject for NonDynamic {
    #[inline(always)]
    fn handle_needed(&mut self, _dynamic_item: DynamicArrayUnion) {}
}

#[derive(Default)]
pub struct Dynamic(Vec<usize>);

impl DynamicObject for Dynamic {
    fn handle_needed(&mut self, dynamic_item: DynamicArrayUnion) {
        self.0.push(unsafe { dynamic_item.d_val });
    }
}

impl Deref for Dynamic {
    type Target = Vec<usize>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Dynamic {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct ObjectBase<T: DynamicObject + Default> {
    pub base: *const c_void,
    pub dynamic_array: *const DynamicArrayItem,
    pub string_table: StringTable,
    pub symbol_table: SymbolTable,
    pub rela_slice: &'static [Rela],
    pub tls_program_header: *const ProgramHeader,
    pub init_array: Option<&'static [InitArrayFunction]>,

    pub needed_libraries: T,
}

impl ObjectBase<NonDynamic> {
    pub unsafe fn build(
        base: *const c_void,
        dynamic_program_header: *const ProgramHeader,
        tls_program_header: *const ProgramHeader,
    ) -> Self {
        Self::build_internal(base, dynamic_program_header, tls_program_header)
    }
}

impl ObjectBase<Dynamic> {
    pub unsafe fn build_dynamic(
        base: *const c_void,
        dynamic_program_header: *const ProgramHeader,
        tls_program_header: *const ProgramHeader,
    ) -> Self {
        Self::build_internal(base, dynamic_program_header, tls_program_header)
    }
}

impl<T: DynamicObject + Default> ObjectBase<T> {
    unsafe fn build_internal(
        base: *const c_void,
        dynamic_program_header: *const ProgramHeader,
        tls_program_header: *const ProgramHeader,
    ) -> Self {
        syscall_debug_assert!(dynamic_program_header != null());

        // Dynamic Arrary:
        let dynamic_array =
            base.byte_add((*dynamic_program_header).p_vaddr) as *const DynamicArrayItem;

        let mut string_table_pointer: *const u8 = null();
        let mut symbol_table_pointer: *const Symbol = null();

        let mut rela_pointer: *const Rela = null();
        let mut rela_count = 0;

        let mut init_array_pointer: *const InitArrayFunction = null();
        let mut init_array_size = 0;

        let mut needed_libraries = T::default();
        for item in DynamicArrayIter::new(dynamic_array) {
            match item.d_tag {
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
            string_table,
            symbol_table,
            rela_slice,
            tls_program_header,
            init_array,
            needed_libraries,
        }
    }
}
