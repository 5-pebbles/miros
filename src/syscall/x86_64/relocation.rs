use std::arch::asm;

use crate::{elf::relocate::Relocatable, io_macros::syscall_assert};

pub unsafe fn relocate(object: &impl Relocatable) {
    let relocation_slices = object.relocation_slices();
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
    const R_X86_64_NONE: u32 = 0;
    /// S + B + A | u64
    const R_X86_64_64: u32 = 1;
    /// S + B + A - P | u32
    const R_X86_64_PC32: u32 = 2;
    /// G + A | u32
    const R_X86_64_GOT32: u32 = 3;
    /// L + A - P | u32
    const R_X86_64_PLT32: u32 = 4;
    /// | None
    const R_X86_64_COPY: u32 = 5;
    /// S + B | u64
    const R_X86_64_GLOB_DAT: u32 = 6;
    /// S + B | u64
    const R_X86_64_JUMP_SLOT: u32 = 7;
    /// B + A | u64
    const R_X86_64_RELATIVE: u32 = 8;
    /// G + GOT + A - P | u32
    const R_X86_64_GOTPCREL: u32 = 9;
    /// S + B + A | u32
    const R_X86_64_32: u32 = 10;
    /// S + B + A | u32
    const R_X86_64_32S: u32 = 11;
    /// S + B + A | u16
    const R_X86_64_16: u32 = 12;
    /// S + B + A - P | u16
    const R_X86_64_PC16: u32 = 13;
    /// S + B + A | u8
    const R_X86_64_8: u32 = 14;
    /// S + B + A - P | u8
    const R_X86_64_PC8: u32 = 15;
    /// S + B + A - P | u64
    const R_X86_64_PC64: u32 = 24;
    /// S + B + A - GOT | u64
    const R_X86_64_GOTOFF64: u32 = 25;
    /// GOT + A - P | u32
    const R_X86_64_GOTPC32: u32 = 26;
    /// Z + A | u32
    const R_X86_64_SIZE32: u32 = 32;
    /// Z + A | u64
    const R_X86_64_SIZE64: u32 = 33;
    /// The returned value from the function located at (B + A) | u64
    const R_X86_64_IRELATIVE: u32 = 37; // This one is fucking awesome... I mean, it's a little annoying but really cool.

    // You may notice some are missing values; those are part of the Thread-Local Storage ABI see "ELF Handling for Thread-Local Storage":
    const R_X86_64_DTPMOD64: u32 = 16;

    for rela in relocation_slices.rela_slice {
        let relocate_address = rela.r_offset.wrapping_add(object.base().addr());

        // x86_64 assembly pointer widths:
        // byte  | 8 bits  (1 byte)
        // word  | 16 bits (2 bytes)
        // dword | 32 bits (4 bytes) | "double word"
        // qword | 64 bits (8 bytes) | "quad word"
        match rela.r_type() {
            R_X86_64_64 => {
                let relocate_value = object
                    .symbol(rela.r_sym() as usize)
                    .st_value
                    .wrapping_add(object.base().addr())
                    .wrapping_add_signed(rela.r_addend);
                asm!(
                    "mov qword ptr [{}], {}",
                    in(reg) relocate_address,
                    in(reg) relocate_value,
                    options(nostack, preserves_flags),
                );
            }
            R_X86_64_GLOB_DAT | R_X86_64_JUMP_SLOT => {
                let relocate_value = object
                    .symbol(rela.r_sym() as usize)
                    .st_value
                    .wrapping_add(object.base().addr());
                asm!(
                    "mov qword ptr [{}], {}",
                    in(reg) relocate_address,
                    in(reg) relocate_value,
                    options(nostack, preserves_flags),
                )
            }
            R_X86_64_RELATIVE => {
                let relocate_value = object.base().addr().wrapping_add_signed(rela.r_addend);
                asm!(
                    "mov qword ptr [{}], {}",
                    in(reg) relocate_address,
                    in(reg) relocate_value,
                    options(nostack, preserves_flags),
                );
            }
            R_X86_64_IRELATIVE => {
                let function_pointer = object.base().addr().wrapping_add_signed(rela.r_addend);
                let function: extern "C" fn() -> usize = core::mem::transmute(function_pointer);
                let relocate_value = function();
                asm!(
                    "mov qword ptr [{}], {}",
                    in(reg) relocate_address,
                    in(reg) relocate_value,
                    options(nostack, preserves_flags),
                );
            }
            _ => {
                syscall_assert!(false, "unsupported relocation");
            }
        }
    }
}
