use bitbybit::{bitenum, bitfield};

use super::section::SectionIndex;

// NOTE: The `Common` SymbolType and SectionIndex are only valid in relocatable objects;
// the static linker resolves these into `.bss` before producing a shared object or executable.

/// Symbol types extracted from the lower nibble of `st_info`.
#[bitenum(u4, exhaustive = false)]
#[derive(PartialEq, Eq)]
pub enum SymbolType {
    NoType = 0,
    Object = 1,
    Function = 2,
    Section = 3,
    File = 4,
    Common = 5,
    Tls = 6,
}

/// Symbol binding types extracted from the upper nibble of `st_info`.
#[bitenum(u4, exhaustive = false)]
#[derive(PartialEq, Eq)]
pub enum SymbolBinding {
    Local = 0,
    Global = 1,
    Weak = 2,
}

#[bitfield(u8)]
pub struct SymbolInfo {
    #[bits(0..=3, rw)]
    symbol_type: Option<SymbolType>,
    #[bits(4..=7, rw)]
    binding: Option<SymbolBinding>,
}

#[bitenum(u2, exhaustive = true)]
#[derive(PartialEq, Eq)]
pub enum SymbolVisibility {
    Default = 0b00,
    Internal = 0b01,
    Hidden = 0b10,
    Protected = 0b11,
}

#[bitfield(u8)]
pub struct SymbolOtherField {
    #[bits(0..=1, rw)]
    symbol_visibility: SymbolVisibility,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Symbol {
    /// String table index of the symbol name.
    pub st_name: u32,
    #[cfg(target_pointer_width = "32")]
    pub st_value: usize,
    #[cfg(target_pointer_width = "32")]
    pub st_size: usize,
    pub st_info: SymbolInfo,
    pub st_other: SymbolOtherField,
    pub st_shndx: u16,
    #[cfg(target_pointer_width = "64")]
    pub st_value: usize,
    #[cfg(target_pointer_width = "64")]
    pub st_size: usize,
}

impl Symbol {
    pub fn section_index(&self) -> Result<SectionIndex, u16> {
        SectionIndex::new_with_raw_value(self.st_shndx)
    }

    pub fn binding(&self) -> Result<SymbolBinding, u8> {
        self.st_info.binding()
    }

    pub fn symbol_type(&self) -> Result<SymbolType, u8> {
        self.st_info.symbol_type()
    }

    pub fn is_defined(&self) -> bool {
        self.st_shndx != 0
    }

    /// Has a known non-local binding. Symbols with OS / processor-specific bindings are conservatively excluded.
    pub fn is_public(&self) -> bool {
        matches!(
            self.binding(),
            Ok(SymbolBinding::Global | SymbolBinding::Weak)
        )
    }

    pub fn is_visible(&self) -> bool {
        matches!(
            self.st_other.symbol_visibility(),
            SymbolVisibility::Default | SymbolVisibility::Protected
        )
    }

    pub fn is_exported(&self) -> bool {
        self.is_visible() && self.is_defined() && self.is_public()
    }
}

pub struct SymbolTable(*const Symbol);

impl SymbolTable {
    pub fn new(symbol_table_pointer: *const Symbol) -> Self {
        Self(symbol_table_pointer)
    }

    pub unsafe fn get(&self, index: usize) -> Symbol {
        *self.0.add(index)
    }

    pub fn into_inner(self) -> *const Symbol {
        self.0
    }
}
