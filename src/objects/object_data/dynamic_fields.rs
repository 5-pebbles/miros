use std::{ffi::c_void, ptr, ptr::null, ptr::NonNull};

use crate::elf::dynamic_array::{DT_GNU_HASH, DT_HASH, DT_NEEDED, DT_PLTGOT, DT_RPATH, DT_RUNPATH};
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
        relocate::Rela,
        string_table::StringTable,
        symbol::{Symbol, SymbolTable},
    },
    io_macros::syscall_debug_assert,
};

use super::{
    hash_tables::HashTable, path_resolver::PathResolver, AnyDynamic, Dynamic, InitArrayFunction,
};

pub struct DynamicFields<T: AnyDynamic> {
    pub global_offset_table: *const usize,
    pub string_table: StringTable,
    pub symbol_table: SymbolTable,
    rela_slice: *const [Rela],
    init_array: Option<*const [InitArrayFunction]>,
    pub hash_table: Option<HashTable>,
    pub path_resolver: PathResolver,
    pub needed_libraries: T,
}

impl DynamicFields<Dynamic> {
    pub fn dependency_names(&self) -> impl Iterator<Item = &str> {
        self.needed_libraries
            .iter()
            .map(|needed_library_index| unsafe { self.string_table.get(*needed_library_index) })
    }
}

impl<T: AnyDynamic> DynamicFields<T> {
    pub unsafe fn rela_slice(&self) -> &[Rela] {
        &*self.rela_slice
    }

    pub unsafe fn init_functions(&self) -> Option<&[InitArrayFunction]> {
        self.init_array.map(|pointer| &*pointer)
    }

    pub(super) unsafe fn from_dynamic_array(
        base: *const c_void,
        dynamic_array: *const DynamicArrayItem,
    ) -> Self {
        let mut global_offset_table_pointer: *const usize = null();
        let mut string_table_pointer: *const u8 = null();
        let mut symbol_table_pointer: *const Symbol = null();

        let mut rela_pointer: *const Rela = null();
        let mut rela_count = 0;

        let mut init_array_pointer: *const InitArrayFunction = null();
        let mut init_array_size = 0;

        let mut hash_table: Option<HashTable> = None;

        let mut rpath_string_table_index: Option<usize> = None;
        let mut runpath_string_table_index: Option<usize> = None;

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

                DT_HASH => {
                    hash_table.get_or_insert(HashTable::from_sysv(base, item.d_un.d_ptr));
                }
                DT_GNU_HASH => hash_table = Some(HashTable::from_gnu(base, item.d_un.d_ptr)),

                DT_RPATH => rpath_string_table_index = Some(item.d_un.d_val),
                DT_RUNPATH => runpath_string_table_index = Some(item.d_un.d_val),

                DT_NEEDED => {
                    needed_libraries.handle_needed(item.d_un);
                }
                _ => (),
            }
        }

        let string_table = StringTable::new(string_table_pointer);
        let symbol_table = SymbolTable::new(symbol_table_pointer);

        let rela_slice = ptr::slice_from_raw_parts(rela_pointer, rela_count);

        let init_array = if init_array_pointer.is_null() || init_array_size == 0 {
            None
        } else {
            Some(ptr::slice_from_raw_parts(
                init_array_pointer,
                init_array_size,
            ))
        };

        let path_resolver = runpath_string_table_index
            .map(|index| PathResolver::Runpath(string_table.get(index)))
            .or(rpath_string_table_index.map(|index| PathResolver::Rpath(string_table.get(index))))
            .unwrap_or(PathResolver::None);

        Self {
            global_offset_table: global_offset_table_pointer,
            string_table,
            symbol_table,
            rela_slice,
            init_array,
            hash_table,
            path_resolver,
            needed_libraries,
        }
    }
}
