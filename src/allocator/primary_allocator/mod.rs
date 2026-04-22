use crate::allocator::primary_allocator::{
    class_region::ClassRegion, non_crypto_rng::Xoroshiro128PlusPlus, size_classes::SIZE_CLASS_COUNT,
};

mod class_region;
mod large_allocator;
mod non_crypto_rng;
mod size_classes;
mod span;

pub struct PrimaryAllocator {
    page_size: usize,
    super_base: *mut u8,
    class_regions: [ClassRegion; SIZE_CLASS_COUNT],
    rng: Xoroshiro128PlusPlus,
}
