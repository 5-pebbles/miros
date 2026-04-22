mod class_region;
mod large_allocator;
mod metadata_allocator;
mod non_crypto_rng;
mod primary;
mod size_classes;
mod span;

use std::mem::MaybeUninit;

use self::primary::PrimaryAllocator;
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
