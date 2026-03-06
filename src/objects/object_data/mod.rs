mod dynamic_fields;
mod dynamic_trait_objects;
mod hash_tables;
mod path_resolver;
mod thread_local;

pub use dynamic_fields::DynamicFields;
pub use dynamic_trait_objects::{AnyDynamic, Dynamic, NonDynamic};
pub use thread_local::{ThreadLocalAllocation, ThreadLocalData};

use std::{ffi::c_void, ptr, ptr::null};

use crate::elf::dynamic_array::DynamicArrayItem;
use crate::elf::header::ElfHeader;
use crate::elf::program_header::{ProgramHeader, PT_DYNAMIC, PT_PHDR, PT_TLS};
use crate::error::MirosError;
use crate::io_macros::syscall_debug_assert;
use crate::start::auxiliary_vector::AuxiliaryVectorItem;

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
