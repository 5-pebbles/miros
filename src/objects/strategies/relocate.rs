use std::{arch::asm, ffi::c_void};

use crate::{
    elf::relocate::Rela,
    error::MirosError,
    objects::{
        object_data::{Dynamic, NonDynamic, ObjectData},
        strategies::{AsObjectDataSlice, Stratagem},
    },
};

// TODO: This could probably benefit from specialization: https://github.com/rust-lang/rust/issues/31844
trait Relocatable {
    fn base(&self) -> Result<*const c_void, MirosError>;
}

impl Relocatable for &ObjectData<NonDynamic> {
    fn base(&self) -> Result<*const c_void, MirosError> {
        Ok(self.base)
    }
}

impl Relocatable for &ObjectData<Dynamic> {
    fn base(&self) -> Result<*const c_void, MirosError> {
        Ok(self.base)
    }
}

pub struct Relocate {}

impl Relocate {
    pub fn new() -> Self {
        Self {}
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn rela(&self, rela: Rela, object_data: impl Relocatable) -> Result<(), MirosError> {
        let relocate_address = rela.r_offset.wrapping_add(object_data.base()?.addr());

        // x86_64 assembly pointer widths:
        // byte  | 8 bits  (1 byte)
        // word  | 16 bits (2 bytes)
        // dword | 32 bits (4 bytes) | "double word"
        // qword | 64 bits (8 bytes) | "quad word"
        use crate::elf::relocate::{R_X86_64_IRELATIVE, R_X86_64_RELATIVE};
        match rela.r_type() {
            R_X86_64_RELATIVE => {
                let relocate_value = object_data
                    .base()?
                    .addr()
                    .wrapping_add_signed(rela.r_addend);
                asm!(
                    "mov qword ptr [{}], {}",
                    in(reg) relocate_address,
                    in(reg) relocate_value,
                    options(nostack, preserves_flags),
                );
            }
            R_X86_64_IRELATIVE => {
                let function_pointer = object_data
                    .base()?
                    .addr()
                    .wrapping_add_signed(rela.r_addend);
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

impl<T: AsObjectDataSlice> Stratagem<T> for Relocate
// SAFETY: DynamicObject is a sealed trait — its only implementors are NonDynamic and Dynamic, so this bound is satisfied for all possible ObjectData variants.
// plus this is checked at compile time... ┌(▀Ĺ̯▀)┐
where
    for<'a> &'a ObjectData<<T as AsObjectDataSlice>::Item>: Relocatable,
{
    fn run(&self, object_data: &mut T) -> Result<(), MirosError> {
        object_data
            .as_object_slice_mut()
            .iter()
            .try_for_each(|object| {
                unsafe { object.dynamic_fields.rela_slice() }
                    .iter()
                    .try_for_each(|rela| unsafe { self.rela(*rela, object) })
            })
    }
}
