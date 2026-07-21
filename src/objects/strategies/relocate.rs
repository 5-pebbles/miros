use std::{arch::asm, ptr};

use crate::{
    elf::{relocate::Rela, symbol::SymbolBinding},
    error::MirosError,
    objects::{object_data::ObjectData, object_data_graph::ObjectDataGraph, strategies::Stratagem},
};

pub struct Relocate {}

impl Relocate {
    pub fn new() -> Self {
        Self {}
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn rela(
        &self,
        rela: Rela,
        object_data: &ObjectData,
        object_data_map: &ObjectDataGraph,
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

                let remote_address = object_data_map
                    .resolve_symbol_address(local_symbol, object_data)
                    .or_else(|err| match local_symbol.binding() {
                        Ok(SymbolBinding::Weak) => Ok(std::ptr::null()),
                        _ => Err(err),
                    })?;

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

                let Some((source_symbol, source_address)) =
                    object_data_map.resolve_symbol_outside_program(symbol_name)
                else {
                    // Undefined weak leaves the destination zeroed, as glibc does; strong is fatal.
                    return match local_symbol.binding() {
                        Ok(SymbolBinding::Weak) => Ok(()),
                        _ => Err(MirosError::UndefinedSymbol(symbol_name.to_string())),
                    };
                };

                // Sizes can disagree after a re-link; the destination's reservation caps the copy.
                ptr::copy_nonoverlapping(
                    source_address.cast::<u8>(),
                    relocate_address as *mut u8,
                    source_symbol.st_size.min(local_symbol.st_size),
                );
            }

            _ => (),
        }

        Ok(())
    }
}

impl Stratagem for Relocate {
    fn run(&self, object_data_map: &mut ObjectDataGraph) -> Result<(), MirosError> {
        // Dependencies before the program: a COPY reloc reads its source object's relocated bytes.
        object_data_map
            .iter_objects_topological()
            .try_for_each(|object| {
                let rela_entries = object.dynamic_fields.rela_slice().unwrap_or(&[]);
                let plt_rela_entries = object.dynamic_fields.plt_rela_slice().unwrap_or(&[]);

                rela_entries
                    .iter()
                    .chain(plt_rela_entries.iter())
                    .try_for_each(|rela| unsafe { self.rela(*rela, object, object_data_map) })
            })
    }
}
