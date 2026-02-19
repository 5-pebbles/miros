use std::ffi::c_void;

use strum::FromRepr;

use crate::{elf::program_header::ProgramHeader, error::MirosError};

#[derive(Debug, FromRepr, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum AuxiliaryVectorType {
    Null = 0,
    Phdr = 3,
    Phent = 4,
    Phnum = 5,
    PageSize = 6,
    Base = 7,
    Entry = 9,
    Random = 25,
}

/// A union resolved by the a_type field of the parent auxiliary vector item.
#[repr(C)]
#[derive(Clone, Copy)]
pub union AuxiliaryVectorUnion {
    pub a_val: usize,
    pub a_ptr: *mut c_void,
}

/// An item in the auxiliary vector.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AuxiliaryVectorItem {
    a_type: usize,
    // NOTE: I couldn't find good documentation on this field; glibc's `getauxval` returns a usize, but I think it really represents union.
    pub a_un: AuxiliaryVectorUnion,
}

impl AuxiliaryVectorItem {
    pub fn a_type(self) -> Result<AuxiliaryVectorType, usize> {
        AuxiliaryVectorType::from_repr(self.a_type).ok_or(self.a_type)
    }
}

pub struct AuxiliaryVectorInfo {
    pub base: *const c_void,
    pub entry: *const c_void,
    pub page_size: usize,
    pub pseudorandom_bytes: *const [u8; 16],
    pub program_header_pointer: *const ProgramHeader,
    pub program_header_count: usize,
}

impl AuxiliaryVectorInfo {
    /// Initializes a new `AuxiliaryVectorIter` from a 16-byte aligned and pre-offset `*const AuxiliaryVectorItem` pointer.
    pub unsafe fn new(auxv_pointer: *const AuxiliaryVectorItem) -> Result<Self, MirosError> {
        let mut base: Result<*const c_void, MirosError> =
            Err(MirosError::MissingAuxvEntry(AuxiliaryVectorType::Base));
        let mut entry: Result<*const c_void, MirosError> =
            Err(MirosError::MissingAuxvEntry(AuxiliaryVectorType::Entry));
        let mut page_size: Result<usize, MirosError> =
            Err(MirosError::MissingAuxvEntry(AuxiliaryVectorType::PageSize));
        let mut pseudorandom_bytes: Result<*const [u8; 16], MirosError> =
            Err(MirosError::MissingAuxvEntry(AuxiliaryVectorType::Random));
        let mut program_header_pointer: Result<*const ProgramHeader, MirosError> =
            Err(MirosError::MissingAuxvEntry(AuxiliaryVectorType::Phdr));
        let mut program_header_count: Result<usize, MirosError> =
            Err(MirosError::MissingAuxvEntry(AuxiliaryVectorType::Phnum));

        (0..)
            .map(|i| *auxv_pointer.add(i))
            .take_while(|item| item.a_type() != Ok(AuxiliaryVectorType::Null))
            .for_each(|item| match item.a_type() {
                Ok(AuxiliaryVectorType::Base) => base = Ok(item.a_un.a_ptr.cast()),
                Ok(AuxiliaryVectorType::Entry) => entry = Ok(item.a_un.a_ptr.cast()),
                Ok(AuxiliaryVectorType::PageSize) => page_size = Ok(item.a_un.a_val),
                Ok(AuxiliaryVectorType::Random) => pseudorandom_bytes = Ok(item.a_un.a_ptr.cast()),
                Ok(AuxiliaryVectorType::Phdr) => {
                    program_header_pointer = Ok(item.a_un.a_ptr.cast())
                }
                Ok(AuxiliaryVectorType::Phnum) => program_header_count = Ok(item.a_un.a_val),
                _ => (),
            });

        Ok(Self {
            base: base?,
            entry: entry?,
            page_size: page_size?,
            pseudorandom_bytes: pseudorandom_bytes?,
            program_header_pointer: program_header_pointer?,
            program_header_count: program_header_count?,
        })
    }
}
