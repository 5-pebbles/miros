use std::ffi::c_void;

pub const DT_NULL: usize = 0;
pub const DT_NEEDED: usize = 1;
pub const DT_PLTRELSZ: usize = 2;
pub const DT_PLTGOT: usize = 3;
pub const DT_HASH: usize = 4;
pub const DT_STRTAB: usize = 5;
pub const DT_SYMTAB: usize = 6;
pub const DT_RELA: usize = 7;
pub const DT_RELASZ: usize = 8;
pub const DT_RELAENT: usize = 9;
pub const DT_SYMENT: usize = 11;
pub const DT_INIT: usize = 12;
pub const DT_FINI: usize = 13;
pub const DT_RPATH: usize = 15;
pub const DT_REL: usize = 17;
pub const DT_TEXTREL: usize = 22;
pub const DT_INIT_ARRAY: usize = 25;
pub const DT_FINI_ARRAY: usize = 26;
pub const DT_INIT_ARRAYSZ: usize = 27;
pub const DT_FINI_ARRAYSZ: usize = 28;
pub const DT_RUNPATH: usize = 29;
pub const DT_RELRSZ: usize = 35;
pub const DT_RELR: usize = 36;

pub const DT_GNU_HASH: usize = 0x6ffffef5;

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
    pub d_tag: usize,
    pub d_un: DynamicArrayUnion,
}

/// An iterator over a `DT_NULL` terminated list of dynamic array items.
///
/// The inital pointer can be found in one of two ways:
/// 1. The base address + an offset in bytes equivalent to the `p_vaddr` field on the `PT_DYNAMIC` entry in the program header table.
/// 2. Via inline asm and the `_DYNAMIC` symbol example:
///
/// ```no_run
/// asm!(
///   "lea {}, [rip + _DYNAMIC]",
///   out(reg) address,
/// );
/// ```
#[derive(Clone, Copy)]
pub struct DynamicArrayIter(*const DynamicArrayItem);

impl DynamicArrayIter {
    /// Initializes a new `DynamicArrayIter` from an initial `*const DynamicArrayItem` pointer.
    pub fn new(dynamic_array_pointer: *const DynamicArrayItem) -> Self {
        Self(dynamic_array_pointer)
    }

    /// Extracts the inner pointer to the next item consuming the `DynamicArrayIter`.
    pub fn into_inner(self) -> *const DynamicArrayItem {
        self.0
    }
}

impl Iterator for DynamicArrayIter {
    type Item = DynamicArrayItem;

    fn next(&mut self) -> Option<Self::Item> {
        let item = unsafe { *self.0 };

        // If we are at the end of the list, return `None` and don't progress.
        if item.d_tag == DT_NULL {
            return None;
        }

        // Advance to the next item
        self.0 = unsafe { self.0.add(1) };

        Some(item)
    }
}
