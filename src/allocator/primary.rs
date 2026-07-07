use std::{
    alloc::Layout,
    mem::MaybeUninit,
    ptr::{self, null_mut, NonNull},
    sync::Mutex,
};

use super::{
    class_region::{ClassRegion, CLASS_REGION_SHIFT, CLASS_REGION_SIZE},
    heap::get_heap,
    large_allocator::LargeAllocator,
    size_classes::{SizeClass, SIZE_CLASS_COUNT},
    ANONYMOUS_PRIVATE_MAP, GUARD_PAGE_PROTECTION,
};
use crate::libc::mem::{mmap, munmap};

const SUPER_REGION_WINDOW_COUNT: usize = SIZE_CLASS_COUNT.next_power_of_two();
const SUPER_REGION_SIZE: usize = CLASS_REGION_SIZE * SUPER_REGION_WINDOW_COUNT;

unsafe fn reserve_super_region() -> *mut u8 {
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

/// The shared half: every mutable field sits behind a lock, so threads route through `&self`.
pub struct PrimaryAllocator {
    super_base: *mut u8,
    class_regions: [ClassRegion; SIZE_CLASS_COUNT],
    large_allocator: Mutex<LargeAllocator>,
    pseudorandom_bytes: u128,
}

impl PrimaryAllocator {
    pub unsafe fn new(pseudorandom_bytes: [u8; 16]) -> Self {
        let super_base = reserve_super_region();

        let mut class_regions: [MaybeUninit<ClassRegion>; SIZE_CLASS_COUNT] =
            [const { MaybeUninit::uninit() }; SIZE_CLASS_COUNT];

        (0..SIZE_CLASS_COUNT).for_each(|class_index| {
            let size_class = SizeClass::from_raw(class_index as u8);
            let base = unsafe { super_base.add(class_index * CLASS_REGION_SIZE) };
            class_regions[class_index].write(unsafe { ClassRegion::new(size_class, base) });
        });

        let class_regions = MaybeUninit::array_assume_init(class_regions);

        Self {
            super_base,
            class_regions,
            large_allocator: Mutex::new(LargeAllocator::new()),
            pseudorandom_bytes: u128::from_ne_bytes(pseudorandom_bytes),
        }
    }

    pub fn class_regions(&self) -> &[ClassRegion; SIZE_CLASS_COUNT] {
        &self.class_regions
    }

    pub fn pseudorandom_bytes(&self) -> u128 {
        self.pseudorandom_bytes
    }

    #[inline(always)]
    pub unsafe fn alloc(&self, layout: Layout) -> Option<NonNull<u8>> {
        match SizeClass::from_layout(layout.size(), layout.align()) {
            Some(size_class) => self.alloc_small(size_class),
            None => self.alloc_large(layout),
        }
    }

    #[inline(always)]
    unsafe fn alloc_small(&self, size_class: SizeClass) -> Option<NonNull<u8>> {
        get_heap()
            .alloc_small(&self.class_regions, size_class)
            .or_else(|| {
                // Window exhausted: satisfy the request from the large path instead of failing.
                let fallback_layout =
                    Layout::from_size_align_unchecked(size_class.slot_size_in_bytes(), 1);
                self.alloc_large(fallback_layout)
            })
    }

    #[inline(always)]
    unsafe fn alloc_large(&self, layout: Layout) -> Option<NonNull<u8>> {
        self.large_allocator
            .lock()
            .unwrap_unchecked()
            .alloc_large(layout)
    }

    #[inline(always)]
    pub unsafe fn alloc_zeroed(&self, layout: Layout) -> Option<NonNull<u8>> {
        match SizeClass::from_layout(layout.size(), layout.align()) {
            Some(size_class) => {
                let pointer = self.alloc_small(size_class)?;
                ptr::write_bytes(pointer.as_ptr(), 0, size_class.slot_size_in_bytes());
                Some(pointer)
            }
            None => self.alloc_large_zeroed(layout),
        }
    }

    #[inline(always)]
    unsafe fn alloc_large_zeroed(&self, layout: Layout) -> Option<NonNull<u8>> {
        self.large_allocator
            .lock()
            .unwrap_unchecked()
            .alloc_large_zeroed(layout)
    }

    #[inline(always)]
    pub unsafe fn free(&self, pointer: *mut u8) {
        if pointer.is_null() {
            return;
        }
        match self.class_from_pointer(pointer) {
            Some(size_class) => self.dealloc_small(pointer, size_class),
            None => self.dealloc_large(pointer),
        }
    }

    pub unsafe fn dealloc(&self, pointer: *mut u8, _layout: Layout) {
        self.free(pointer)
    }

    #[inline(always)]
    unsafe fn dealloc_small(&self, pointer: *mut u8, size_class: SizeClass) {
        let region = &self.class_regions[size_class.index()];
        let span_node = region.span_for_pointer(pointer);
        let span = &(*span_node).value;

        let heap = get_heap();
        if span.owner() == heap.id() {
            heap.dealloc_local(region, size_class, pointer);
        } else {
            span.remote_dealloc_slot(pointer);
        }
    }

    #[inline(always)]
    unsafe fn dealloc_large(&self, pointer: *mut u8) {
        self.large_allocator
            .lock()
            .unwrap_unchecked()
            .dealloc_large(pointer)
    }

    pub unsafe fn realloc(&self, pointer: *mut u8, new_size: usize) -> Option<NonNull<u8>> {
        if pointer.is_null() {
            return self.alloc(Layout::from_size_align_unchecked(new_size, 1));
        }
        if new_size == 0 {
            self.free(pointer);
            return None;
        }

        let old_class = self.class_from_pointer(pointer);
        let new_class = SizeClass::from_layout(new_size, 1);

        if old_class.is_some() && old_class == new_class {
            // Same class: the existing block already satisfies the request.
            return Some(NonNull::new_unchecked(pointer));
        }

        let new_pointer = match new_class {
            Some(size_class) => self.alloc_small(size_class),
            None => self.alloc_large(Layout::from_size_align_unchecked(new_size, 1)),
        }?;

        let old_usable = match old_class {
            Some(class) => class.slot_size_in_bytes(),
            None => self
                .large_allocator
                .lock()
                .unwrap_unchecked()
                .allocation_size(pointer),
        };

        copy_realloc_payload(
            pointer,
            new_pointer.as_ptr(),
            old_usable.min(new_size),
            old_class,
        );

        match old_class {
            Some(size_class) => self.dealloc_small(pointer, size_class),
            None => self.dealloc_large(pointer),
        }
        Some(new_pointer)
    }

    #[inline(always)]
    fn class_from_pointer(&self, pointer: *mut u8) -> Option<SizeClass> {
        let offset = pointer.addr().wrapping_sub(self.super_base.addr());
        // One bound covers both the super-region edge and the unused padding windows past class 13.
        if offset >= SIZE_CLASS_COUNT * CLASS_REGION_SIZE {
            return None;
        }
        let class_index = offset >> CLASS_REGION_SHIFT;
        Some(SizeClass::from_raw(class_index as u8))
    }
}

#[inline(always)]
unsafe fn copy_realloc_payload(
    source: *const u8,
    dest: *mut u8,
    copy_bytes: usize,
    old_class: Option<SizeClass>,
) {
    if let Some(class) = old_class {
        if copy_bytes == class.slot_size_in_bytes() {
            class.copy_slot(source, dest);
            return;
        }
    }
    ptr::copy_nonoverlapping(source, dest, copy_bytes);
}
