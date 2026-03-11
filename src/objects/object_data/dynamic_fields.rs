use std::{ffi::c_void, ptr};

use super::{hash_tables::HashTable, path_resolver::PathResolver};
#[cfg(debug_assertions)]
use crate::io_macros::syscall_assert;
use crate::{
    elf::{
        dynamic_array::{DynamicArrayItem, DynamicArrayIter, DynamicTag},
        relocate::Rela,
        string_table::StringTable,
        symbol::{Symbol, SymbolTable},
    },
    error::MirosError,
    objects::strategies::init_array::InitArrayFunction,
};

pub struct DynamicFields {
    pub global_offset_table: Option<*const usize>,
    pub string_table: StringTable,
    pub symbol_table: SymbolTable,
    rela_slice: Option<*const [Rela]>,
    plt_rela_slice: Option<*const [Rela]>,
    init_array: Option<*const [InitArrayFunction]>,
    pub hash_table: Option<HashTable>,
    pub path_resolver: PathResolver,
    needed_libraries_string_table_offsets: Vec<usize>,
}

impl DynamicFields {
    pub(super) unsafe fn from_dynamic_array(
        base: *const c_void,
        dynamic_array: *const DynamicArrayItem,
    ) -> Result<Self, MirosError> {
        let mut global_offset_table: Option<*const usize> = None;
        let mut string_table_pointer: Result<*const u8, MirosError> =
            Err(MirosError::MissingDynamicEntry(DynamicTag::StrTab));
        let mut symbol_table_pointer: Result<*const Symbol, MirosError> =
            Err(MirosError::MissingDynamicEntry(DynamicTag::SymTab));

        let mut rela_pointer: Option<*const Rela> = None;
        let mut rela_count = 0;

        let mut plt_rela_pointer: Option<*const Rela> = None;
        let mut plt_rela_count = 0;

        let mut init_array_pointer: Option<*const InitArrayFunction> = None;
        let mut init_array_size = 0;

        let mut hash_table: Option<HashTable> = None;

        let mut rpath_string_table_index: Option<usize> = None;
        let mut runpath_string_table_index: Option<usize> = None;

        let mut needed_libraries_string_table_offsets: Vec<usize> = Vec::new();

        DynamicArrayIter::new(dynamic_array).for_each(|item| match item.d_tag() {
            Ok(DynamicTag::PltGot) => {
                global_offset_table = Some(base.byte_add(item.d_un.d_ptr.addr()) as *const usize)
            }
            Ok(DynamicTag::StrTab) => {
                string_table_pointer = Ok(base.byte_add(item.d_un.d_ptr.addr()) as *const u8)
            }
            Ok(DynamicTag::SymTab) => {
                symbol_table_pointer = Ok(base.byte_add(item.d_un.d_ptr.addr()) as *const Symbol)
            }
            #[cfg(debug_assertions)]
            Ok(DynamicTag::SymEnt) => syscall_assert!(item.d_un.d_val == size_of::<Symbol>()),

            Ok(DynamicTag::Rela) => {
                rela_pointer = Some(base.byte_add(item.d_un.d_ptr.addr()) as *const Rela);
            }
            Ok(DynamicTag::RelaSz) => {
                rela_count = item.d_un.d_val / size_of::<Rela>();
            }
            #[cfg(debug_assertions)]
            Ok(DynamicTag::RelaEnt) => {
                syscall_assert!(item.d_un.d_val == size_of::<Rela>())
            }

            Ok(DynamicTag::JmpRel) => {
                plt_rela_pointer = Some(base.byte_add(item.d_un.d_ptr.addr()) as *const Rela);
            }
            Ok(DynamicTag::PltRelSz) => {
                plt_rela_count = item.d_un.d_val / size_of::<Rela>();
            }
            #[cfg(debug_assertions)]
            Ok(DynamicTag::PltRel) => {
                syscall_assert!(item.d_un.d_val == DynamicTag::Rela as usize)
            }

            Ok(DynamicTag::InitArray) => {
                init_array_pointer =
                    Some(base.byte_add(item.d_un.d_ptr.addr()) as *const InitArrayFunction);
            }
            Ok(DynamicTag::InitArraySz) => {
                init_array_size = item.d_un.d_val / size_of::<usize>();
            }

            Ok(DynamicTag::Hash) => {
                hash_table.get_or_insert(HashTable::from_sysv(base, item.d_un.d_ptr));
            }
            Ok(DynamicTag::GnuHash) => {
                hash_table = Some(HashTable::from_gnu(base, item.d_un.d_ptr))
            }

            Ok(DynamicTag::Rpath) => rpath_string_table_index = Some(item.d_un.d_val),
            Ok(DynamicTag::Runpath) => runpath_string_table_index = Some(item.d_un.d_val),

            Ok(DynamicTag::Needed) => {
                needed_libraries_string_table_offsets.push(item.d_un.d_val);
            }
            _ => (),
        });

        let string_table = StringTable::new(string_table_pointer?);
        let symbol_table = SymbolTable::new(symbol_table_pointer?);

        let rela_slice = rela_pointer.map(|pointer| ptr::slice_from_raw_parts(pointer, rela_count));
        let plt_rela_slice =
            plt_rela_pointer.map(|pointer| ptr::slice_from_raw_parts(pointer, plt_rela_count));

        let init_array =
            init_array_pointer.map(|pointer| ptr::slice_from_raw_parts(pointer, init_array_size));

        let path_resolver = runpath_string_table_index
            .map(|index| PathResolver::Runpath(string_table.get(index)))
            .or(rpath_string_table_index.map(|index| PathResolver::Rpath(string_table.get(index))))
            .unwrap_or(PathResolver::None);

        Ok(Self {
            global_offset_table,
            string_table,
            symbol_table,
            rela_slice,
            plt_rela_slice,
            init_array,
            hash_table,
            path_resolver,
            needed_libraries_string_table_offsets,
        })
    }

    pub fn dependencies(&self) -> impl Iterator<Item = &str> {
        self.needed_libraries_string_table_offsets
            .iter()
            .map(|offset| unsafe { self.string_table.get(*offset) })
    }

    pub unsafe fn rela_slice(&self) -> Option<&[Rela]> {
        self.rela_slice.map(|pointer| &*pointer)
    }

    pub unsafe fn plt_rela_slice(&self) -> Option<&[Rela]> {
        self.plt_rela_slice.map(|pointer| &*pointer)
    }

    pub unsafe fn init_functions(&self) -> Option<&[InitArrayFunction]> {
        self.init_array.map(|pointer| &*pointer)
    }
}
