use std::arch::asm;

use crate::{
    elf::relocate::Rela,
    error::MirosError,
    objects::{object_data::ObjectData, object_data_map::ObjectDataMap, strategies::Stratagem},
};

pub struct Relocate {}

impl Relocate {
    pub fn new() -> Self {
        Self {}
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn rela(&self, rela: Rela, object_data: &ObjectData) -> Result<(), MirosError> {
        let relocate_address = rela.r_offset.wrapping_add(object_data.base.addr());

        // x86_64 assembly pointer widths:
        // byte  | 8 bits  (1 byte)
        // word  | 16 bits (2 bytes)
        // dword | 32 bits (4 bytes) | "double word"
        // qword | 64 bits (8 bytes) | "quad word"
        use crate::elf::relocate::{R_X86_64_IRELATIVE, R_X86_64_RELATIVE};
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
                let function: extern "C" fn() -> usize = core::mem::transmute(function_pointer);
                let relocate_value = function();
                asm!(
                    "mov qword ptr [{}], {}",
                    in(reg) relocate_address,
                    in(reg) relocate_value,
                    options(nostack, preserves_flags),
                );
            }
            _ => (),
        }

        Ok(())
    }
}

impl Stratagem for Relocate {
    fn run(&self, object_data: &mut ObjectDataMap) -> Result<(), MirosError> {
        object_data.iter_objects().try_for_each(|object| {
            unsafe { object.dynamic_fields.rela_slice() }
                .iter()
                .try_for_each(|rela| unsafe { self.rela(*rela, object) })
        })
    }
}
