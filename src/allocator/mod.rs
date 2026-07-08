mod class_region;
mod heap;
mod large_allocator;
mod non_crypto_rng;
mod primary;
mod size_classes;
mod span;

use std::mem::MaybeUninit;

pub(crate) use self::heap::{abandon_heap, install_heap};
use self::{class_region::ClassRegion, primary::PrimaryAllocator, size_classes::SIZE_CLASS_COUNT};
use crate::{
    libc::mem::{MapFlags, ProtectionFlags},
    objects::strategies::init_array::InitArrayFunction,
    start::auxiliary_vector::{AuxiliaryVectorInfo, AuxiliaryVectorItem},
};

pub(crate) const DATA_PAGE_PROTECTION: ProtectionFlags = ProtectionFlags::ZERO
    .with_readable(true)
    .with_writable(true);

pub(crate) const GUARD_PAGE_PROTECTION: ProtectionFlags = ProtectionFlags::ZERO;

pub(crate) const ANONYMOUS_PRIVATE_MAP: MapFlags =
    MapFlags::ZERO.with_private(true).with_anonymous(true);

#[cfg_attr(not(test), link_section = ".init_array")]
#[used]
pub(crate) static INIT_ALLOCATOR: InitArrayFunction = init_allocator;

extern "C" fn init_allocator(
    _arg_count: usize,
    _arg_pointer: *const *const u8,
    _env_pointer: *const *const u8,
    auxv_pointer: *const AuxiliaryVectorItem,
) {
    unsafe {
        let auxv_info = AuxiliaryVectorInfo::new(auxv_pointer).unwrap();

        #[allow(static_mut_refs)]
        PRIMARY.write(PrimaryAllocator::new(*auxv_info.pseudorandom_bytes));
    }
}

pub(crate) static mut PRIMARY: MaybeUninit<PrimaryAllocator> = MaybeUninit::uninit();

#[inline(always)]
pub(crate) unsafe fn primary() -> &'static PrimaryAllocator {
    #[allow(static_mut_refs)]
    PRIMARY.assume_init_ref()
}

unsafe fn global_class_regions() -> &'static [ClassRegion; SIZE_CLASS_COUNT] {
    primary().class_regions()
}

unsafe fn pseudorandom_bytes() -> u128 {
    primary().pseudorandom_bytes()
}
