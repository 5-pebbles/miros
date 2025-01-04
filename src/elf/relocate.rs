use core::ffi::c_void;

use crate::elf::symbol::Symbol;

pub trait Relocatable {
    fn base(&self) -> *const ();
    fn symbol(&self, symbol_index: usize) -> Symbol;
    fn relocation_slices(&self) -> RelocationSlices;
}

#[derive(Clone, Copy)]
pub struct RelocationSlices {
    pub rela_slice: &'static [Rela],
}

/// An ELF relocation entry with an addend.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Rela {
    pub r_offset: usize,
    pub r_info: usize,
    pub r_addend: isize,
}

impl Rela {
    /// Extracts the symbol table index from the `r_info` field.
    pub fn r_sym(&self) -> u32 {
        #[cfg(target_pointer_width = "64")]
        {
            (self.r_info >> 32) as u32
        }
        #[cfg(target_pointer_width = "32")]
        {
            (self.r_info >> 8) as u32
        }
    }

    /// Extracts the relocation type from the `r_info` field.
    pub fn r_type(&self) -> u32 {
        #[cfg(target_pointer_width = "64")]
        {
            (self.r_info & 0xFFFFFFFF) as u32
        }
        #[cfg(target_pointer_width = "32")]
        {
            (self.r_info & 0xFF) as u32
        }
    }
}

#[cfg(target_arch = "x86_64")]
pub mod relocations {
    // Variables in relocation formulae:
    // - A(rela.r_addend): This is the addend used to compute the value of the relocatable field.
    // - B(self.base.addr): This is the base address at which a shared object has been loaded into memory during execution.
    // - G(??): This is the offset into the global offset table at which the address of the relocation entry’s symbol will reside during execution.
    // - GOT(global_offset_table_address): This is the address of the global offset table.
    // - L(??): ??
    // - P(relocate_address): This is the address of the storage unit being relocated.
    // - S(self.symbol.st_value): This is the value of the symbol table entry indexed at `rela.r_sym()`.
    //   NOTE: In the ELF specification `S` is equal to (symbol.st_value + base_address) but that doesn't make any sense to me.
    // - Z(??): ??

    // x86_64 relocation types:
    /// | None
    pub const R_X86_64_NONE: u32 = 0;
    /// S + B + A | u64
    pub const R_X86_64_64: u32 = 1;
    /// S + B + A - P | u32
    pub const R_X86_64_PC32: u32 = 2;
    /// G + A | u32
    pub const R_X86_64_GOT32: u32 = 3;
    /// L + A - P | u32
    pub const R_X86_64_PLT32: u32 = 4;
    /// | None
    pub const R_X86_64_COPY: u32 = 5;
    /// S + B | u64
    pub const R_X86_64_GLOB_DAT: u32 = 6;
    /// S + B | u64
    pub const R_X86_64_JUMP_SLOT: u32 = 7;
    /// B + A | u64
    pub const R_X86_64_RELATIVE: u32 = 8;
    /// G + GOT + A - P | u32
    pub const R_X86_64_GOTPCREL: u32 = 9;
    /// S + B + A | u32
    pub const R_X86_64_32: u32 = 10;
    /// S + B + A | u32
    pub const R_X86_64_32S: u32 = 11;
    /// S + B + A | u16
    pub const R_X86_64_16: u32 = 12;
    /// S + B + A - P | u16
    pub const R_X86_64_PC16: u32 = 13;
    /// S + B + A | u8
    pub const R_X86_64_8: u32 = 14;
    /// S + B + A - P | u8
    pub const R_X86_64_PC8: u32 = 15;
    /// S + B + A - P | u64
    pub const R_X86_64_PC64: u32 = 24;
    /// S + B + A - GOT | u64
    pub const R_X86_64_GOTOFF64: u32 = 25;
    /// GOT + A - P | u32
    pub const R_X86_64_GOTPC32: u32 = 26;
    /// Z + A | u32
    pub const R_X86_64_SIZE32: u32 = 32;
    /// Z + A | u64
    pub const R_X86_64_SIZE64: u32 = 33;
    /// The returned value from the function located at (B + A) | u64
    pub const R_X86_64_IRELATIVE: u32 = 37; // This one is fucking awesome... I mean, it's a little annoying but really cool.

    // You may notice some are missing values; those are part of the Thread-Local Storage ABI see "ELF Handling for Thread-Local Storage":
    pub const R_X86_64_DTPMOD64: u32 = 16;
}

pub use relocations::*;
