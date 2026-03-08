mod private {
    use super::{Dynamic, NonDynamic};

    pub trait Sealed {}
    impl Sealed for NonDynamic {}
    impl Sealed for Dynamic {}
}

pub trait DynamicObject: private::Sealed {
    fn only_if_dynamic(f: impl FnOnce());
    fn only_if_nondynamic(f: impl FnOnce());
}

#[derive(Default)]
pub struct NonDynamic;

impl DynamicObject for NonDynamic {
    #[inline(always)]
    fn only_if_dynamic(_f: impl FnOnce()) {}
    fn only_if_nondynamic(f: impl FnOnce()) {
        f()
    }
}

#[derive(Default)]
pub struct Dynamic;

impl DynamicObject for Dynamic {
    fn only_if_dynamic(f: impl FnOnce()) {
        f()
    }
    #[inline(always)]
    fn only_if_nondynamic(_f: impl FnOnce()) {}
}

pub trait AnyDynamic = DynamicObject + Default;
