use std::arch::asm;
use std::ffi::c_void;
use std::fmt::Debug;
use std::marker::PhantomData;

use crate::elf::relocate::Rela;

#[derive(Debug)]
pub struct RelocationError<T: Debug> {
    relocation_type: PhantomData<T>,
}

pub trait RelaRelocatable {
    type RelaError = RelocationError<Rela>;
    fn base(&self) -> Result<*const c_void, Self::RelaError>;
    unsafe fn rela_relocate(
        &self,
        rela_iter: impl IntoIterator<Item = &Rela>,
    ) -> Result<(), Self::RelaError> {
        #[cfg(target_arch = "x86_64")]
        for rela in rela_iter {
            let relocate_address = rela.r_offset.wrapping_add(self.base()?.addr());

            // x86_64 assembly pointer widths:
            // byte  | 8 bits  (1 byte)
            // word  | 16 bits (2 bytes)
            // dword | 32 bits (4 bytes) | "double word"
            // qword | 64 bits (8 bytes) | "quad word"
            use crate::elf::relocate::{R_X86_64_IRELATIVE, R_X86_64_RELATIVE};
            match rela.r_type() {
                R_X86_64_RELATIVE => {
                    let relocate_value = self.base()?.addr().wrapping_add_signed(rela.r_addend);
                    asm!(
                        "mov qword ptr [{}], {}",
                        in(reg) relocate_address,
                        in(reg) relocate_value,
                        options(nostack, preserves_flags),
                    );
                }
                R_X86_64_IRELATIVE => {
                    let function_pointer = self.base()?.addr().wrapping_add_signed(rela.r_addend);
                    let function: extern "C" fn() -> usize = core::mem::transmute(function_pointer);
                    let relocate_value = function();
                    asm!(
                        "mov qword ptr [{}], {}",
                        in(reg) relocate_address,
                        in(reg) relocate_value,
                        options(nostack, preserves_flags),
                    );
                }
                _ => todo!(),
            }
        }
        Ok(())
    }
}
