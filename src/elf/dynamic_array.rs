use std::ffi::c_void;

use strum::FromRepr;

#[derive(Debug, FromRepr, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum DynamicTag {
    Null = 0,
    Needed = 1,
    PltRelSz = 2,
    PltGot = 3,
    Hash = 4,
    StrTab = 5,
    SymTab = 6,
    Rela = 7,
    RelaSz = 8,
    RelaEnt = 9,
    SymEnt = 11,
    Init = 12,
    Fini = 13,
    Rpath = 15,
    Rel = 17,
    PltRel = 20,
    TextRel = 22,
    JmpRel = 23,
    InitArray = 25,
    FiniArray = 26,
    InitArraySz = 27,
    FiniArraySz = 28,
    Runpath = 29,
    PreInitArray = 32,
    PreInitArraySz = 33,
    RelrSz = 35,
    Relr = 36,
    GnuHash = 0x6ffffef5,
}

/// A union resolved by the d_tag field of the parent dynamic array item.
#[repr(C)]
#[derive(Copy, Clone)]
pub union DynamicArrayUnion {
    pub d_val: usize,
    pub d_ptr: *mut c_void,
}

/// An item in the dynamic array.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DynamicArrayItem {
    d_tag: usize,
    pub d_un: DynamicArrayUnion,
}

impl DynamicArrayItem {
    pub fn d_tag(self) -> Result<DynamicTag, usize> {
        DynamicTag::from_repr(self.d_tag).ok_or(self.d_tag)
    }
}

pub struct DynamicArrayIter {
    current: *const DynamicArrayItem,
}

impl DynamicArrayIter {
    pub unsafe fn new(start: *const DynamicArrayItem) -> Self {
        Self { current: start }
    }
}

impl Iterator for DynamicArrayIter {
    type Item = DynamicArrayItem;

    fn next(&mut self) -> Option<Self::Item> {
        let item = unsafe { *self.current };
        if item.d_tag() == Ok(DynamicTag::Null) {
            return None;
        }
        self.current = unsafe { self.current.add(1) };
        Some(item)
    }
}
