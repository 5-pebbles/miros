use std::ffi::c_void;

use crate::{
    allocator::{
        metadata_allocator::MetadataAllocator,
        primary_allocator::{
            class_region::ClassRegion, non_crypto_rng::Xoroshiro128PlusPlus,
            size_classes::SIZE_CLASS_COUNT, span::Span,
        },
    },
    linked_list::LinkedListNode,
};

mod class_region;
mod large_allocator;
mod non_crypto_rng;
mod size_classes;
mod span;

pub struct PrimaryAllocator {
    page_size: usize,
    super_base: *mut c_void,
    class_regions: [ClassRegion; SIZE_CLASS_COUNT],
    span_metadata: MetadataAllocator<LinkedListNode<Span>>,
    rng: Xoroshiro128PlusPlus,
}
