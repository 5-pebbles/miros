use std::{ffi::c_void, marker::PhantomData, ptr};

use super::{
    hash_tables::HashTable, path_resolver::PathResolver, AnyDynamic, Dynamic, InitArrayFunction,
};
#[cfg(debug_assertions)]
use crate::io_macros::syscall_assert;
use crate::{
    elf::{
        dynamic_array::{DynamicArrayItem, DynamicTag},
        relocate::Rela,
        string_table::StringTable,
        symbol::{Symbol, SymbolTable},
    },
    error::MirosError,
    objects::object_data_map::LibraryNameHash,
};

pub struct NeededLibrary {
    pub string_table_offset: usize,
    pub hash: LibraryNameHash,
}

pub struct DynamicFields<T: AnyDynamic> {
    pub global_offset_table: *const usize,
    pub string_table: StringTable,
    pub symbol_table: SymbolTable,
    rela_slice: *const [Rela],
    init_array: Option<*const [InitArrayFunction]>,
    pub hash_table: Option<HashTable>,
    pub path_resolver: PathResolver,
    pub needed_libraries: Vec<NeededLibrary>,
    _marker: PhantomData<T>,
}

impl DynamicFields<Dynamic> {
    pub fn dependency_names(&self) -> impl Iterator<Item = &str> {
        self.needed_libraries
            .iter()
            .map(|library| unsafe { self.string_table.get(library.string_table_offset) })
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
    ) -> Result<Self, MirosError> {
        let mut global_offset_table_pointer: Result<*const usize, MirosError> =
            Err(MirosError::MissingDynamicEntry(DynamicTag::PltGot));
        let mut string_table_pointer: Result<*const u8, MirosError> =
            Err(MirosError::MissingDynamicEntry(DynamicTag::StrTab));
        let mut symbol_table_pointer: Result<*const Symbol, MirosError> =
            Err(MirosError::MissingDynamicEntry(DynamicTag::SymTab));

        let mut rela_pointer: Result<*const Rela, MirosError> =
            Err(MirosError::MissingDynamicEntry(DynamicTag::Rela));
        let mut rela_count = 0;

        let mut init_array_pointer: *const InitArrayFunction = ptr::null();
        let mut init_array_size = 0;

        let mut hash_table: Option<HashTable> = None;

        let mut rpath_string_table_index: Option<usize> = None;
        let mut runpath_string_table_index: Option<usize> = None;

        let mut needed_offsets: Vec<usize> = Vec::new();

        (0..)
            .map(|index| *dynamic_array.add(index))
            .take_while(|item| item.d_tag() != Ok(DynamicTag::Null))
            .for_each(|item| match item.d_tag() {
                Ok(DynamicTag::PltGot) => {
                    global_offset_table_pointer =
                        Ok(base.byte_add(item.d_un.d_ptr.addr()) as *const usize)
                }
                Ok(DynamicTag::StrTab) => {
                    string_table_pointer = Ok(base.byte_add(item.d_un.d_ptr.addr()) as *const u8)
                }
                Ok(DynamicTag::SymTab) => {
                    symbol_table_pointer =
                        Ok(base.byte_add(item.d_un.d_ptr.addr()) as *const Symbol)
                }
                #[cfg(debug_assertions)]
                Ok(DynamicTag::SymEnt) => syscall_assert!(item.d_un.d_val == size_of::<Symbol>()),

                Ok(DynamicTag::Rela) => {
                    rela_pointer = Ok(base.byte_add(item.d_un.d_ptr.addr()) as *const Rela);
                }
                Ok(DynamicTag::RelaSz) => {
                    rela_count = item.d_un.d_val / core::mem::size_of::<Rela>();
                }
                #[cfg(debug_assertions)]
                Ok(DynamicTag::RelaEnt) => {
                    syscall_assert!(item.d_un.d_val == size_of::<Rela>())
                }

                Ok(DynamicTag::InitArray) => {
                    init_array_pointer =
                        base.byte_add(item.d_un.d_ptr.addr()) as *const InitArrayFunction;
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
                    T::only_if_dynamic(|| needed_offsets.push(item.d_un.d_val));
                }
                _ => (),
            });

        let string_table = StringTable::new(string_table_pointer?);
        let symbol_table = SymbolTable::new(symbol_table_pointer?);

        let rela_slice = ptr::slice_from_raw_parts(rela_pointer?, rela_count);

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

        let needed_libraries = needed_offsets
            .into_iter()
            .map(|offset| NeededLibrary {
                string_table_offset: offset,
                hash: LibraryNameHash::new(string_table.get(offset)),
            })
            .collect();

        Ok(Self {
            global_offset_table: global_offset_table_pointer?,
            string_table,
            symbol_table,
            rela_slice,
            init_array,
            hash_table,
            path_resolver,
            needed_libraries,
            _marker: PhantomData,
        })
    }
}
