use std::{
    alloc::Layout,
    mem::MaybeUninit,
    ptr::{self, null_mut},
};

use super::{
    class_region::{ClassRegion, CLASS_REGION_SHIFT, CLASS_REGION_SIZE},
    large_allocator::LargeAllocator,
    non_crypto_rng::Xoroshiro128PlusPlus,
    size_classes::{SizeClass, SIZE_CLASS_COUNT},
    ANONYMOUS_PRIVATE_MAP, GUARD_PAGE_PROTECTION,
};
use crate::libc::mem::{mmap, munmap};

/// Number of class-region windows in the super-region.
/// Rounded to the next power of two so pointer→class is a single shift.
/// (the padding window is never touched — pure virtual address space)
const SUPER_REGION_WINDOW_COUNT: usize = SIZE_CLASS_COUNT.next_power_of_two();
const SUPER_REGION_SIZE: usize = CLASS_REGION_SIZE * SUPER_REGION_WINDOW_COUNT;

/// Reserve a `SUPER_REGION_SIZE` + aligned block of virtual address space.
unsafe fn reserve_super_region() -> *mut u8 {
    // Over-allocate 2x the target, so we can find the aligned base within that.
    let raw = mmap(
        null_mut(),
        SUPER_REGION_SIZE * 2,
        GUARD_PAGE_PROTECTION,
        ANONYMOUS_PRIVATE_MAP,
        -1,
        0,
    );
    assert!((raw as isize) > 0, "super-region reservation failed");

    let raw_address = raw.addr();
    let aligned_address = (raw_address + SUPER_REGION_SIZE - 1) & !(SUPER_REGION_SIZE - 1);
    let leading_slack = aligned_address - raw_address;
    let trailing_slack = SUPER_REGION_SIZE - leading_slack;

    if leading_slack > 0 {
        munmap(raw, leading_slack);
    }
    if trailing_slack > 0 {
        munmap(raw.add(leading_slack + SUPER_REGION_SIZE), trailing_slack);
    }

    let aligned = raw.add(leading_slack);
    debug_assert!(aligned.addr() % SUPER_REGION_SIZE == 0);
    aligned
}

pub struct PrimaryAllocator {
    super_base: *mut u8,
    class_regions: [ClassRegion; SIZE_CLASS_COUNT],
    large_allocator: LargeAllocator,
    rng: Xoroshiro128PlusPlus,
}

impl PrimaryAllocator {
    pub unsafe fn new(seed: [u8; 16]) -> Self {
        let super_base = reserve_super_region();

        // Can't use array::from_fn because ClassRegion::new is unsafe.
        let mut class_regions: [MaybeUninit<ClassRegion>; SIZE_CLASS_COUNT] =
            [const { MaybeUninit::uninit() }; SIZE_CLASS_COUNT];

        (0..SIZE_CLASS_COUNT).for_each(|class_index| {
            let size_class = SizeClass::from_raw(class_index as u8);
            let base = super_base.add(class_index * CLASS_REGION_SIZE);
            class_regions[class_index].write(ClassRegion::new(size_class, base));
        });

        // SAFETY: every element was initialized in the loop above.
        let class_regions = MaybeUninit::array_assume_init(class_regions);

        Self {
            super_base,
            class_regions,
            large_allocator: LargeAllocator::new(),
            rng: Xoroshiro128PlusPlus::from_bytes(u128::from_ne_bytes(seed)),
        }
    }

    // ── allocation ──────────────────────────────────────────────────────

    #[inline(always)]
    pub unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        match SizeClass::from_layout(layout.size(), layout.align()) {
            Some(size_class) => self.alloc_small(size_class),
            None => self.large_allocator.alloc_large(layout),
        }
    }

    #[inline(always)]
    unsafe fn alloc_small(&mut self, size_class: SizeClass) -> *mut u8 {
        let random = self.rng.next_u64();
        let result = self.class_regions[size_class.index()].alloc_slot(random);
        if !result.is_null() {
            return result;
        }
        // Region's 16 GB address space exhausted - fall back to large path.
        let fallback_layout = Layout::from_size_align_unchecked(size_class.slot_size_in_bytes(), 1);
        self.large_allocator.alloc_large(fallback_layout)
    }

    #[inline(always)]
    pub unsafe fn alloc_zeroed(&mut self, layout: Layout) -> *mut u8 {
        match SizeClass::from_layout(layout.size(), layout.align()) {
            Some(size_class) => {
                let pointer = self.alloc_small(size_class);
                if !pointer.is_null() {
                    ptr::write_bytes(pointer, 0, size_class.slot_size_in_bytes());
                }
                pointer
            }
            None => self.large_allocator.alloc_large_zeroed(layout),
        }
    }

    // ── deallocation ────────────────────────────────────────────────────

    /// Entry point for the C `free(ptr)` — no layout information available,
    /// so the class is derived entirely from the pointer's address.
    #[inline(always)]
    pub unsafe fn free(&mut self, pointer: *mut u8) {
        if pointer.is_null() {
            return;
        }
        match self.class_from_pointer(pointer) {
            Some(size_class) => self.dealloc_small(pointer, size_class),
            None => self.large_allocator.dealloc_large(pointer),
        }
    }

    /// Entry point for Rust's `GlobalAlloc::dealloc` — has layout but we route
    /// by pointer address for consistency with `free`.
    pub unsafe fn dealloc(&mut self, pointer: *mut u8, _layout: Layout) {
        self.free(pointer);
    }

    #[inline(always)]
    unsafe fn dealloc_small(&mut self, pointer: *mut u8, size_class: SizeClass) {
        self.class_regions[size_class.index()].dealloc_slot(pointer);
    }

    // ── reallocation ────────────────────────────────────────────────────

    /// C-ABI `realloc(ptr, new_size)` — derives old layout entirely from the pointer.
    pub unsafe fn realloc(&mut self, pointer: *mut u8, new_size: usize) -> *mut u8 {
        if pointer.is_null() {
            return self.alloc(Layout::from_size_align_unchecked(new_size, 1));
        }
        if new_size == 0 {
            self.free(pointer);
            return null_mut();
        }

        let old_class = self.class_from_pointer(pointer);
        let new_class = SizeClass::from_layout(new_size, 1);

        // Same size class — the existing slot already fits.
        if old_class.is_some() && old_class == new_class {
            return pointer;
        }

        let old_usable = match old_class {
            Some(class) => class.slot_size_in_bytes(),
            None => self.large_allocator.allocation_size(pointer),
        };

        let new_pointer = self.alloc(Layout::from_size_align_unchecked(new_size, 1));
        if new_pointer.is_null() {
            return null_mut();
        }

        ptr::copy_nonoverlapping(pointer, new_pointer, old_usable.min(new_size));
        self.free(pointer);
        new_pointer
    }

    // ── pointer classification ──────────────────────────────────────────

    /// O(1) pointer → size class.
    /// Returns `None` for pointers outside the super-region (i.e. large allocations or foreign pointers).
    #[inline(always)]
    fn class_from_pointer(&self, pointer: *mut u8) -> Option<SizeClass> {
        let offset = pointer.addr().wrapping_sub(self.super_base.addr());
        if offset >= SUPER_REGION_SIZE {
            return None;
        }
        let class_index = offset >> CLASS_REGION_SHIFT;
        if class_index >= SIZE_CLASS_COUNT {
            return None;
        }
        Some(SizeClass::from_raw(class_index as u8))
    }
}
