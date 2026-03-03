use std::ops::{Deref, DerefMut};

use crate::elf::dynamic_array::DynamicArrayUnion;

mod private {
    use super::{Dynamic, NonDynamic};

    pub trait Sealed {}
    impl Sealed for NonDynamic {}
    impl Sealed for Dynamic {}
}

pub trait DynamicObject: private::Sealed {
    fn handle_needed(&mut self, dynamic_item: DynamicArrayUnion);
}

#[derive(Default)]
pub struct NonDynamic;

impl DynamicObject for NonDynamic {
    #[inline(always)]
    fn handle_needed(&mut self, _dynamic_item: DynamicArrayUnion) {}
}

#[derive(Default)]
pub struct Dynamic(Vec<usize>);

impl DynamicObject for Dynamic {
    fn handle_needed(&mut self, dynamic_item: DynamicArrayUnion) {
        self.0.push(unsafe { dynamic_item.d_val });
    }
}

impl Deref for Dynamic {
    type Target = Vec<usize>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Dynamic {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub trait AnyDynamic = DynamicObject + Default;
