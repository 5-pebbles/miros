use core::{slice, str};

/// A collection of null-terminated strings stored in contiguous memory.
///
/// The initial pointer can be found via the `DT_STRTAB` entry in the dynamic array. The first and last index are guaranteed to be null.
/// To get the string at index `i`, start at the `i`th byte and read until a null byte is encountered.
///
/// The following shows a string table with 38 bytes and example string locations:
/// ```no_run
/// |    |  0  |  1  |  2  |  3  |  4  |  5  |  6  |  7  |  8  |  9  |
/// |:--:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
/// | 0x | \0  |  H  |  e  |  l  |  l  |  o  | \0  |  W  |  o  |  r  |
/// | 1x |  l  |  d  |  !  | \0  |  T  |  h  |  a  |  n  |  k  |  s  |
/// | 2x | \0  |  f  |  o  |  r  | \0  |  A  |  l  |  l  | \0  |  t  |
/// | 3x |  h  |  e  | \0  |  F  |  i  |  s  |  h  | \0  |     |     |
/// ```
///
/// Example string lookups:
/// ```no_run
/// | Index | String |
/// |:-----:|--------|
/// |   0   |  None  |
/// |   1   |  Hello |
/// |   3   |  llo   |
/// |   32  |  None  |
/// |   33  |  Fish  |
/// ```
pub struct StringTable(*const u8);

impl StringTable {
    /// Creates a new `StringTable` from a `*const u8` pointer to the start of the string table.
    pub fn new(string_table_pointer: *const u8) -> Self {
        Self(string_table_pointer)
    }

    /// Retrieves a string from the table at the specified byte offset.
    pub unsafe fn get(&self, index: usize) -> &str {
        let string_start = self.0.add(index);
        let length = (0..).find(|&index| *string_start.add(index) == 0).unwrap();
        str::from_utf8_unchecked(slice::from_raw_parts(string_start, length))
    }

    /// Extracts the inner pointer to the next item consuming the `StringTable`.
    pub fn into_inner(self) -> *const u8 {
        self.0
    }
}
