use core::ffi::c_void;

use super::environment_variables::EnvironmentIter;

pub const AT_NULL: usize = 0;
pub const AT_PHDR: usize = 3;
pub const AT_PHENT: usize = 4;
pub const AT_PHNUM: usize = 5;
pub const AT_PAGE_SIZE: usize = 6;
pub const AT_BASE: usize = 7;
pub const AT_ENTRY: usize = 9;
pub const AT_RANDOM: usize = 25;

/// A union resolved by the a_type field of the parent auxiliary vector item.
#[repr(C)]
#[derive(Clone, Copy)]
pub union AuxiliaryVectorUnion {
    pub a_val: usize,
    pub a_ptr: *mut (),
}

/// An item in the auxiliary vector.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AuxiliaryVectorItem {
    pub a_type: usize,
    // NOTE: I couldn't find good documentation on this field; glibc's `getauxval` returns a usize, but I think it really represents union.
    pub a_un: AuxiliaryVectorUnion,
}

/// An iterator over a `AT_NULL` terminated list of auxiliary vector items.
///
/// The inital pointer can be found two null-bytes after the end of the environmant pointers:
///
/// ```no_run
/// |---------------------|
/// | arg_count           |
/// |---------------------|
/// | arg_values...       |
/// |---------------------|
/// | null                |
/// |---------------------|
/// | env_pointers...     |
/// |---------------------|
/// | null                |
/// |---------------------|
/// | null                |
/// |---------------------|
/// | auxiliary_vector... |
/// |---------------------|
/// | null                |
/// |---------------------|
/// | ...                 |
/// |---------------------|
/// ```
#[derive(Clone, Copy)]
pub struct AuxiliaryVectorIter(*const AuxiliaryVectorItem);

impl AuxiliaryVectorIter {
    /// Initializes a new `AuxiliaryVectorIter` from a 16-byte aligned and pre-offset `*const AuxiliaryVectorItem` pointer.
    pub unsafe fn new(auxiliary_vector_pointer: *const AuxiliaryVectorItem) -> Self {
        Self(auxiliary_vector_pointer)
    }

    /// Calculates and initializes a new `AuxiliaryVectorIter` from an `EnvironmentIter`.
    pub unsafe fn from_env_pointer(env_pointer: impl Into<*const *const u8>) -> Self {
        let mut env_pointer = env_pointer.into();

        unsafe {
            while !(*env_pointer).is_null() {
                env_pointer = env_pointer.add(1);
            }

            Self::new(env_pointer.add(1) as *const AuxiliaryVectorItem)
        }
    }

    /// Extracts the inner pointer to the next item consuming the `AuxiliaryVectorIter`.
    pub fn into_inner(self) -> *const AuxiliaryVectorItem {
        self.0
    }
}

impl Iterator for AuxiliaryVectorIter {
    type Item = AuxiliaryVectorItem;

    fn next(&mut self) -> Option<Self::Item> {
        let item = unsafe { *self.0 };

        // If we are at the end of the list, return `None` and don't progress.
        if item.a_type == AT_NULL {
            return None;
        }

        // Advance to the next item
        self.0 = unsafe { self.0.add(1) };

        Some(item)
    }
}
