use std::{
    ptr::NonNull,
    sync::atomic::{Atomic, AtomicU64, Ordering},
};

use crate::allocator::{
    class_region::ClassRegion,
    heap::{class_heap::ThreadClassHeap, magazine::Magazines},
    non_crypto_rng::HeapRng,
    size_classes::{SizeClass, SIZE_CLASS_COUNT},
};

/// Identifies a heap. Monotonic and never reused, so a dead thread's spans never alias a live heap's id.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HeapId(u64);

/// A span's owner cell: any thread loads it to route a free, the owner stores it on adoption.
pub struct AtomicHeapId(Atomic<u64>);

impl AtomicHeapId {
    pub fn new(id: HeapId) -> Self {
        Self(Atomic::<u64>::new(id.0))
    }

    pub fn load(&self, order: Ordering) -> HeapId {
        HeapId(self.0.load(order))
    }

    pub fn store(&self, id: HeapId, order: Ordering) {
        self.0.store(id.0, order);
    }
}

// ⌊2^64 / φ⌋ forced odd: the multiply stays invertible (distinct ids never collide) and consecutive ids scatter maximally far apart.
const GOLDEN_RATIO_MULTIPLIER: u64 = 0x9E37_79B9_7F4A_7C15;

/// One per live thread, reached through a `#[thread_local]` pointer.
pub struct Heap {
    heap_id: HeapId,
    classes: [ThreadClassHeap; SIZE_CLASS_COUNT],
    magazines: Magazines,
    rng: HeapRng,
}

impl Heap {
    pub fn new(pseudorandom_bytes: u128) -> Self {
        static NEXT_HEAP_ID: AtomicU64 = AtomicU64::new(1);

        let raw_id = NEXT_HEAP_ID.fetch_add(1, Ordering::Relaxed);

        // Per-thread stream: mix the heap id into the process seed so threads don't share a draw sequence.
        // `| 1` keeps the xoroshiro state non-zero.
        let mixed = (raw_id as u128).wrapping_mul(GOLDEN_RATIO_MULTIPLIER as u128);
        let seed = (pseudorandom_bytes ^ (mixed << 64) ^ mixed) | 1;

        Self {
            heap_id: HeapId(raw_id),
            classes: [const { ThreadClassHeap::new() }; SIZE_CLASS_COUNT],
            magazines: Magazines::new(),
            rng: HeapRng::from_bytes(seed),
        }
    }

    pub fn id(&self) -> HeapId {
        self.heap_id
    }

    #[inline(always)]
    pub unsafe fn alloc_small(
        &mut self,
        global_regions: &[ClassRegion; SIZE_CLASS_COUNT],
        size_class: SizeClass,
    ) -> Option<NonNull<u8>> {
        // Refill at the low-water mark so the draw always has a wide pool to randomize over.
        if self.magazines.needs_refill(size_class) {
            self.refill_class(size_class, &global_regions[size_class.index()]);
        }
        self.magazines.class(size_class).draw_random(&mut self.rng)
    }

    #[cold]
    #[inline(never)]
    unsafe fn refill_class(&mut self, size_class: SizeClass, region: &ClassRegion) {
        let mut magazine = self.magazines.class(size_class);
        self.classes[size_class.index()].refill(&mut magazine, region, self.heap_id, &mut self.rng);
    }

    /// Stage the free in the magazine; a full magazine spills back to the span bitmap, in bulk.
    #[inline(always)]
    pub unsafe fn dealloc_local(
        &mut self,
        region: &ClassRegion,
        size_class: SizeClass,
        pointer: *mut u8,
    ) {
        let mut magazine = self.magazines.class(size_class);
        if !magazine.try_push(pointer) {
            self.classes[size_class.index()].flush_to_span(&mut magazine, region);
            // Flush drains to the low-water mark, so a slot is always free here.
            let pushed = magazine.try_push(pointer);
            debug_assert!(pushed, "magazine full immediately after flush");
        }
    }

    /// On thread exit, flush magazines back to their span bitmaps (still marked taken there), then abandon the spans.
    pub unsafe fn abandon_all(&mut self, global_regions: &[ClassRegion; SIZE_CLASS_COUNT]) {
        for class_index in 0..SIZE_CLASS_COUNT {
            let global = &global_regions[class_index];

            let mut magazine = self.magazines.class(SizeClass::from_raw(class_index as u8));
            while let Some(pointer) = magazine.pop() {
                let span_node = global.span_for_pointer(pointer);
                span_node.as_ref().value.dealloc_slot(pointer);
            }

            self.classes[class_index].abandon_all(global);
        }
    }
}
