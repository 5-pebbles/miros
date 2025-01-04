use core::{slice, str};

use crate::io_macros::*;

/// An iterator over a null-terminated list of pointers to embedded-null-byte-terminated strings representing environment variables.
///
/// The inital pointer can be found one byte after the end of the argument slice, which is passed to the process by the kernel via the stack pointer:
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
/// | ...                 |
/// |---------------------|
/// ```
#[derive(Clone, Copy)]
pub struct EnvironmentIter(*mut *mut u8);

impl EnvironmentIter {
    /// Initializes a new `EnvironmentIter` from a 16-byte aligned and pre-offset `*mut *mut u8` pointer.
    pub fn new(environment_pointer: *mut *mut u8) -> Self {
        unsafe {
            // Ensure we are below the argument slice (or at least something null) and 16-byte aligned.
            syscall_debug_assert!(environment_pointer.addr() & 0b1111 == 0);
            syscall_debug_assert!((*environment_pointer.sub(1)).is_null());
        };

        Self(environment_pointer)
    }

    /// Calculates the offset and initializes a new `EnvironmentIter` from a 16-byte aligned `*const usize` stack pointer.
    pub fn from_stack_pointer(stack_pointer: *const usize) -> Self {
        // Ensure that `stack_pointer` is not null and 16-byte aligned.
        syscall_debug_assert!(stack_pointer != core::ptr::null_mut());
        syscall_debug_assert!(stack_pointer.addr() & 0b1111 == 0);

        unsafe {
            let argument_count = *stack_pointer as usize;
            let argument_pointer = stack_pointer.add(1) as *mut *mut u8;
            syscall_debug_assert!((*argument_pointer.add(argument_count)).is_null());

            Self(argument_pointer.add(argument_count + 1))
        }
    }

    /// Extracts the inner pointer to the next item consuming the `EnvironmentIter`.
    pub fn into_inner(self) -> *mut *mut u8 {
        self.0
    }
}

impl Iterator for EnvironmentIter {
    type Item = (&'static str, &'static str);

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let string_pointer = *self.0;

            // If we are at the end of the list, return `None` and don't progress.
            if string_pointer.is_null() {
                return None;
            }

            let mut split_at = None;
            let end_index = (0..).find(|&index| {
                let character_byte = *string_pointer.add(index);
                // Update `split_at` on the first `b'='`.
                if split_at.is_none() && character_byte == b'=' {
                    split_at = Some(index);
                }
                character_byte == 0
            })?;

            // Ensure the variable is not malformed.
            syscall_debug_assert!(split_at.is_some());
            let split_at = split_at.unwrap();

            let name_slice = slice::from_raw_parts(string_pointer, split_at);
            let value_slice =
                slice::from_raw_parts(string_pointer.add(split_at + 1), end_index - split_at - 1);

            // The validity check segfaults in this context. :/
            // This is the same as just calling `mem::transmute`.
            let name = str::from_utf8_unchecked(name_slice);
            let value = str::from_utf8_unchecked(value_slice);

            // Advance to the next string pointer.
            self.0 = self.0.add(1);

            Some((name, value))
        }
    }
}
