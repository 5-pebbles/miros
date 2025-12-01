use bitbybit::{bitenum, bitfield};

#[bitenum(u2, exhaustive = true)]
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
    pub st_info: u8,
    pub st_other: SymbolOtherField,
    pub st_shndx: u16,
    #[cfg(target_pointer_width = "64")]
    pub st_value: usize,
    #[cfg(target_pointer_width = "64")]
    pub st_size: usize,
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
