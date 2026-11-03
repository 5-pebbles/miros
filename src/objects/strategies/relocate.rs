use std::{arch::asm, ptr};

use crate::{
    elf::{
        relocate::{Rela, Relr},
        symbol::SymbolBinding,
    },
    error::MirosError,
    objects::{object_data::ObjectData, object_data_graph::ObjectDataGraph, strategies::Stratagem},
    start::config::{RuntimeConfig, UndefinedSymbolsPolicy},
};

pub struct Relocate;

impl Relocate {
    /// Adds the load base to each selected word. Address entries also set the starting point for subsequent bitmap entries.
    #[cfg(target_arch = "x86_64")]
    unsafe fn relr(&self, object_data: &ObjectData) {
        let Some(entries) = object_data.dynamic_fields.relr_slice() else {
            return;
        };

        let base = object_data.base.addr();
        let mut location: *mut usize = ptr::null_mut();

        for &entry in entries {
            match Relr::from(entry) {
                Relr::Address(offset) => {
                    location = (base + offset) as *mut usize;
                    *location = (*location).wrapping_add(base);
                    location = location.add(1);
                }
                Relr::Bitmap(bitmap) => {
                    // Bitmap 0b101: [word 0 += base] [word 1] [word 2 += base].
                    for bit in Relr::set_bits(bitmap) {
                        let slot = location.add(bit);
                        *slot = (*slot).wrapping_add(base);
                    }
                    location = location.add(Relr::BITMAP_WORD_COUNT);
                }
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn rela(
        &self,
        rela: Rela,
        object_data: &ObjectData,
        object_data_map: &ObjectDataGraph,
        undefined_symbols: &mut Vec<String>,
    ) -> Result<(), MirosError> {
        let relocate_address = rela.r_offset.wrapping_add(object_data.base.addr());

        // x86_64 assembly pointer widths:
        // byte  | 8 bits  (1 byte)
        // word  | 16 bits (2 bytes)
        // dword | 32 bits (4 bytes) | "double word"
        // qword | 64 bits (8 bytes) | "quad word"
        use crate::elf::relocate::{
            R_X86_64_COPY, R_X86_64_GLOB_DAT, R_X86_64_IRELATIVE, R_X86_64_JUMP_SLOT,
            R_X86_64_RELATIVE,
        };
        match rela.r_type() {
            R_X86_64_RELATIVE => {
                let relocate_value = object_data.base.addr().wrapping_add_signed(rela.r_addend);
                asm!(
                    "mov qword ptr [{}], {}",
                    in(reg) relocate_address,
                    in(reg) relocate_value,
                    options(nostack, preserves_flags),
                );
            }
            R_X86_64_IRELATIVE => {
                let function_pointer = object_data.base.addr().wrapping_add_signed(rela.r_addend);
                let function: extern "C" fn() -> usize = std::mem::transmute(function_pointer);
                let relocate_value = function();
                asm!(
                    "mov qword ptr [{}], {}",
                    in(reg) relocate_address,
                    in(reg) relocate_value,
                    options(nostack, preserves_flags),
                );
            }
            R_X86_64_GLOB_DAT | R_X86_64_JUMP_SLOT => {
                debug_assert_eq!(rela.r_addend, 0);

                let local_symbol = object_data
                    .dynamic_fields
                    .checked_symbol(rela.r_sym() as usize)?;

                let symbol_name = object_data
                    .dynamic_fields
                    .string_table
                    .get(local_symbol.st_name as usize);

                let resolved = object_data_map.resolve_symbol_address(symbol_name, object_data);
                let remote_address = resolved.unwrap_or_default();

                // Undefined symbols relocate to null: weak by spec, strong only under lenient mode.
                if resolved.is_none() && local_symbol.binding() != Ok(SymbolBinding::Weak) {
                    undefined_symbols.push(symbol_name.to_string());
                }

                asm!(
                    "mov qword ptr [{}], {}",
                    in(reg) relocate_address,
                    in(reg) remote_address,
                    options(nostack, preserves_flags),
                );
            }

            R_X86_64_COPY => {
                let local_symbol = object_data
                    .dynamic_fields
                    .checked_symbol(rela.r_sym() as usize)?;
                let symbol_name = object_data
                    .dynamic_fields
                    .string_table
                    .get(local_symbol.st_name as usize);

                if let Some((source_symbol, source_address)) =
                    object_data_map.resolve_symbol_outside_program(symbol_name)
                {
                    // Sizes can disagree after a re-link; the destination's reservation caps the copy.
                    ptr::copy_nonoverlapping(
                        source_address.cast::<u8>(),
                        relocate_address as *mut u8,
                        source_symbol.st_size.min(local_symbol.st_size),
                    );
                } else if local_symbol.binding() != Ok(SymbolBinding::Weak) {
                    undefined_symbols.push(symbol_name.to_string());
                };
            }

            _ => (),
        }

        Ok(())
    }
}

impl Stratagem for Relocate {
    fn run(&self, object_data_map: &mut ObjectDataGraph) -> Result<(), MirosError> {
        let mut undefined_symbols = Vec::<String>::new();
        // Dependencies before the program: a COPY reloc reads its source object's relocated bytes.
        for object in object_data_map.iter_objects_topological() {
            unsafe { self.relr(object) };

            let rela_entries = object.dynamic_fields.rela_slice().unwrap_or_default();
            let plt_rela_entries = object.dynamic_fields.plt_rela_slice().unwrap_or_default();
            for rela in rela_entries.iter().chain(plt_rela_entries) {
                unsafe { self.rela(*rela, object, object_data_map, &mut undefined_symbols) }?;
            }
        }

        if undefined_symbols.is_empty() {
            return Ok(());
        }

        if RuntimeConfig::get().undefined_symbols == UndefinedSymbolsPolicy::Lenient {
            eprintln!("{}", MirosError::UndefinedSymbols(undefined_symbols));
            Ok(())
        } else {
            Err(MirosError::UndefinedSymbols(undefined_symbols))
        }
    }
}
