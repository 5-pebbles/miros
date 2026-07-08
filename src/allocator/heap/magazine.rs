use std::ptr::{null_mut, NonNull};

use crate::allocator::{
    non_crypto_rng::HeapRng,
    size_classes::{SizeClass, SIZE_CLASSES, SIZE_CLASS_COUNT},
};

// TODO: I just picked these numbers at random, if anyone wants to make a more educated choice go for it.
pub const MAX_MAGAZINE_CAPACITY: usize = 64;
pub const MIN_MAGAZINE_CAPACITY: usize = 8;

pub const MAX_MAGAZINE_BYTES: usize = 4096;

type EntryIndex = u16;
const _: () = assert!(MAX_MAGAZINE_CAPACITY <= EntryIndex::MAX as usize);

#[derive(Copy, Clone)]
pub struct MagazineLayout {
    pub capacity: u16,
    pub low_water: u16,
    pub offset: u16,
}

impl MagazineLayout {
    const fn magazine_capacity(slot_size_in_bytes: usize) -> usize {
        (MAX_MAGAZINE_BYTES / slot_size_in_bytes)
            .clamp(MIN_MAGAZINE_CAPACITY, MAX_MAGAZINE_CAPACITY)
    }

    /// Refill trigger; lower mark -> larger batches, fewer span queries.
    const fn magazine_low_water(capacity: u16) -> u16 {
        // TODO: Arbitrary fraction for now.
        capacity / 4
    }

    /// Capacity + packed-buffer offset for every class, prefix-summed once at compile time.
    pub const MAGAZINE_LAYOUT: [MagazineLayout; SIZE_CLASS_COUNT] = {
        let mut layout = [MagazineLayout {
            capacity: 0,
            low_water: 0,
            offset: 0,
        }; SIZE_CLASS_COUNT];
        let mut offset: u16 = 0;
        let mut index = 0;
        while index < SIZE_CLASS_COUNT {
            let capacity = Self::magazine_capacity(SIZE_CLASSES[index].slot_size_in_bytes) as u16;
            let low_water = Self::magazine_low_water(capacity);
            layout[index] = MagazineLayout {
                capacity,
                low_water,
                offset,
            };

            offset += capacity;
            index += 1;
        }
        layout
    };

    pub const MAGAZINE_TOTAL_CAPACITY: usize = unsafe {
        let MagazineLayout {
            capacity, offset, ..
        } = *MagazineLayout::MAGAZINE_LAYOUT.last().unwrap_unchecked();
        (offset + capacity) as usize
    };
}

// Both a very adept name, and a very american name: (˶ᵔ ᵕᵔ˶)︻╦̵̵̿╤─
pub struct Magazines {
    entries: [*mut u8; MagazineLayout::MAGAZINE_TOTAL_CAPACITY],
    entry_count: [EntryIndex; SIZE_CLASS_COUNT],
}

impl Magazines {
    pub const fn new() -> Self {
        Self {
            entries: [null_mut(); MagazineLayout::MAGAZINE_TOTAL_CAPACITY],
            entry_count: [0; SIZE_CLASS_COUNT],
        }
    }

    #[inline(always)]
    pub(crate) fn class(&mut self, size_class: SizeClass) -> Magazine<'_> {
        let class_index = size_class.index();
        unsafe {
            let MagazineLayout {
                capacity,
                low_water,
                offset,
            } = *MagazineLayout::MAGAZINE_LAYOUT.get_unchecked(class_index);
            // Range slice would recompute the length as `(offset + capacity) - offset`; u16 wrapping blocks the const fold.
            let base = self.entries.as_mut_ptr().add(offset as usize);
            Magazine {
                view: std::slice::from_raw_parts_mut(base, capacity as usize),
                count: self.entry_count.get_unchecked_mut(class_index),
                low_water,
            }
        }
    }

    /// Refill test without materializing the view, so the hot path never spills the `Magazine` struct.
    #[inline(always)]
    pub(crate) fn needs_refill(&self, size_class: SizeClass) -> bool {
        let class_index = size_class.index();
        unsafe {
            let low_water = MagazineLayout::MAGAZINE_LAYOUT
                .get_unchecked(class_index)
                .low_water;
            *self.entry_count.get_unchecked(class_index) <= low_water
        }
    }
}

pub(crate) struct Magazine<'a> {
    // SAFETY: Internally we have to make sure every pushed pointer in slots `[0..count)` are non-null.
    view: &'a mut [*mut u8],
    count: &'a mut u16,
    low_water: u16,
}

impl<'a> Magazine<'a> {
    pub fn remaining_capacity(&self) -> usize {
        self.view.len() - *self.count as usize
    }

    /// Random cached slot via swap-remove: breaks the `free(p); malloc()` reuse primitive without touching the span bitmap. None when empty.
    #[inline(always)]
    pub fn draw_random(&mut self, rng: &mut HeapRng) -> Option<NonNull<u8>> {
        let count = *self.count as usize;
        if count == 0 {
            return None;
        }
        let index = rng.index_below(count);
        let chosen = unsafe { *self.view.get_unchecked(index) };

        *self.count -= 1;
        // Compact the hole with the top slot; the draw was uniform, so this stays unbiased.
        unsafe {
            let top = *self.view.get_unchecked(*self.count as usize);
            *self.view.get_unchecked_mut(index) = top;
        }
        // SAFETY: entries in `[0..count)` are non-null by the magazine's push invariant.
        Some(unsafe { NonNull::new_unchecked(chosen) })
    }

    #[inline(always)]
    fn pop_above(&mut self, threshold: u16) -> Option<*mut u8> {
        (*self.count > threshold).then(|| {
            *self.count -= 1;
            unsafe { *self.view.get_unchecked(*self.count as usize) }
        })
    }

    /// Hand back the most-recently-staged slot; None when empty.
    #[inline(always)]
    pub fn pop(&mut self) -> Option<*mut u8> {
        self.pop_above(0)
    }

    /// LIFO pop while above the low-water mark, for draining the overflow back to spans.
    #[inline(always)]
    pub fn pop_above_low_water(&mut self) -> Option<*mut u8> {
        self.pop_above(self.low_water)
    }

    /// Stage a freed slot for handout; false when full, signalling a bulk flush is due.
    #[inline(always)]
    pub fn try_push(&mut self, pointer: *mut u8) -> bool {
        if *self.count as usize == self.view.len() {
            return false;
        }
        unsafe {
            *self.view.get_unchecked_mut(*self.count as usize) = pointer;
        }
        *self.count += 1;
        true
    }

    /// Caller guarantees the claimed count fits the remaining capacity.
    #[inline(always)]
    pub fn refill(&mut self, claimed: impl Iterator<Item = *mut u8>) {
        for pointer in claimed {
            unsafe {
                *self.view.get_unchecked_mut(*self.count as usize) = pointer;
            }
            *self.count += 1;
        }
    }
}
