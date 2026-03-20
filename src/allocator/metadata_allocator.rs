use std::{
    marker::PhantomData,
    mem::size_of,
    ptr::{self, null_mut},
};

use super::{ANONYMOUS_PRIVATE_MAP, DATA_PAGE_PROTECTION, GUARD_PAGE_PROTECTION};
use crate::libc::mem::{mmap, mprotect, munmap};

// Region layout: [Header][Bitmap bytes][align pad][Slot 0][Slot 1]...[Slot N-1]
#[repr(C)]
struct RegionHeader {
    next: *mut RegionHeader,
    prev: *mut RegionHeader,
    slots_occupied: u32,
}

pub struct MetadataAllocator<T> {
    partial_regions: *mut RegionHeader,
    full_regions: *mut RegionHeader,
    page_size: usize,
    slots_per_region: u32,
    slots_offset: usize,
    _marker: PhantomData<T>,
}

impl<T> MetadataAllocator<T> {
    pub unsafe fn new(page_size: usize) -> Self {
        const {
            assert!(
                size_of::<T>() > 0,
                "MetadataAllocator does not support zero-sized types"
            );
        }

        debug_assert!(page_size.is_power_of_two());

        let (slots_per_region, slots_offset) = Self::compute_slot_geometry(page_size);

        Self {
            partial_regions: null_mut(),
            full_regions: null_mut(),
            page_size,
            slots_per_region,
            slots_offset,
            _marker: PhantomData,
        }
    }

    unsafe fn compute_slot_geometry(page_size: usize) -> (u32, usize) {
        let header_size = size_of::<RegionHeader>();
        let slot_size = size_of::<T>();
        let slot_alignment = align_of::<T>();

        let slot_offset_of = |slots_per_region: usize| -> usize {
            let bitmap_bytes = (slots_per_region + 7) / 8;
            (header_size + bitmap_bytes + slot_alignment - 1) & !(slot_alignment - 1)
        };

        let max_maybe_possible_slots_per_region =
            8 * (page_size - header_size) / (8 * slot_size + 1);

        (1..=max_maybe_possible_slots_per_region)
            .rev()
            .find(|&slots_per_region| {
                let slots_offset = slot_offset_of(slots_per_region);
                slots_offset + slots_per_region * slot_size <= page_size
            })
            .map(|slots_per_region| (slots_per_region as u32, slot_offset_of(slots_per_region)))
            .expect("type T is too large to fit in a single page")
    }

    pub fn alloc(&mut self) -> *mut T {
        unsafe {
            if self.partial_regions.is_null() {
                self.create_region();
            }

            let region = self.partial_regions;
            let header = &mut *region;
            let bitmap = Self::bitmap_start(region);

            let slot_index = Self::find_free_slot(
                bitmap,
                (self.slots_per_region + 7) / 8,
                self.slots_per_region,
            );
            let slot_index = match slot_index {
                Some(index) => index,
                None => {
                    debug_assert!(false, "partial region has no free slots");
                    core::hint::unreachable_unchecked()
                }
            };

            let byte_index = (slot_index / u8::BITS) as usize;
            let bit_index = slot_index % u8::BITS;
            let byte = bitmap.add(byte_index);
            *byte |= 1u8 << bit_index;

            header.slots_occupied += 1;

            if header.slots_occupied == self.slots_per_region {
                Self::list_remove(&mut self.partial_regions, region);
                Self::list_push_front(&mut self.full_regions, region);
            }

            let slot_address = (region as *mut u8)
                .add(self.slots_offset)
                .add(slot_index as usize * size_of::<T>());

            slot_address as *mut T
        }
    }

    pub fn dealloc(&mut self, pointer: *mut T) {
        unsafe {
            let region = self.region_from_pointer(pointer);
            let header = &mut *region;
            let bitmap = Self::bitmap_start(region);

            let slots_start = (region as usize) + self.slots_offset;
            let slot_index = ((pointer as usize) - slots_start) / size_of::<T>();

            debug_assert!(
                (slot_index as u32) < self.slots_per_region,
                "slot index out of bounds"
            );

            let byte_index = slot_index / u8::BITS as usize;
            let bit_index = (slot_index % u8::BITS as usize) as u32;
            let byte = bitmap.add(byte_index);

            debug_assert_ne!(*byte & (1u8 << bit_index), 0, "double free detected");

            ptr::drop_in_place(pointer);
            *byte &= !(1u8 << bit_index);

            let was_full = header.slots_occupied == self.slots_per_region;
            header.slots_occupied -= 1;

            if was_full {
                Self::list_remove(&mut self.full_regions, region);
                Self::list_push_front(&mut self.partial_regions, region);
            } else if header.slots_occupied == 0 {
                Self::list_remove(&mut self.partial_regions, region);
                self.destroy_region(region);
            }
        }
    }

    unsafe fn bitmap_start(region: *mut RegionHeader) -> *mut u8 {
        (region as *mut u8).add(size_of::<RegionHeader>())
    }

    unsafe fn find_free_slot(
        bitmap: *mut u8,
        bitmap_byte_count: u32,
        slot_count: u32,
    ) -> Option<u32> {
        for byte_index in 0..bitmap_byte_count as usize {
            let byte = *bitmap.add(byte_index);
            let free_bits = !byte;
            if free_bits != 0 {
                let bit_index = free_bits.trailing_zeros();
                let slot_index = (byte_index as u32) * u8::BITS + bit_index;
                if slot_index < slot_count {
                    return Some(slot_index);
                }
            }
        }
        None
    }

    // SAFETY: Assumes regions are exactly 1 (one) page.
    // Masking off the low bits of a slot pointer recovers the region header
    // only because the entire region fits within a single page-aligned page.
    unsafe fn region_from_pointer(&self, pointer: *mut T) -> *mut RegionHeader {
        let region_mask = !(self.page_size - 1);
        (pointer as usize & region_mask) as *mut RegionHeader
    }

    unsafe fn list_push_front(head: &mut *mut RegionHeader, node: *mut RegionHeader) {
        let old_head = *head;
        (*node).next = old_head;
        (*node).prev = null_mut();
        if let Some(old_head) = old_head.as_mut() {
            old_head.prev = node;
        }
        *head = node;
    }

    unsafe fn list_remove(head: &mut *mut RegionHeader, node: *mut RegionHeader) {
        let prev = (*node).prev;
        let next = (*node).next;

        match prev.as_mut() {
            Some(prev) => prev.next = next,
            None => *head = next,
        }

        if let Some(next) = next.as_mut() {
            next.prev = prev;
        }

        (*node).next = null_mut();
        (*node).prev = null_mut();
    }

    unsafe fn create_region(&mut self) {
        let total_size = self.page_size * 3;

        let region_start = mmap(
            null_mut(),
            total_size,
            GUARD_PAGE_PROTECTION,
            ANONYMOUS_PRIVATE_MAP,
            -1,
            0,
        );

        let usable_page = region_start.add(self.page_size);
        mprotect(usable_page, self.page_size, DATA_PAGE_PROTECTION);

        let usable_start = usable_page as *mut RegionHeader;

        ptr::write(
            usable_start,
            RegionHeader {
                next: null_mut(),
                prev: null_mut(),
                slots_occupied: 0,
            },
        );
        // NOTE: The bitmap bytes are guaranteed zero by the kernel (anonymous mmap pages are always zeroed),
        // which represents all slots free.

        Self::list_push_front(&mut self.partial_regions, usable_start);
    }

    unsafe fn destroy_region(&self, region: *mut RegionHeader) {
        let mmap_start = (region as *mut u8).sub(self.page_size);
        let total_size = self.page_size * 3;
        munmap(mmap_start, total_size);
    }
}
