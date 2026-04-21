mod region;

use std::{
    marker::PhantomData,
    mem::size_of,
    ptr::{self, null_mut},
};

use region::RegionHeader;

use super::{ANONYMOUS_PRIVATE_MAP, DATA_PAGE_PROTECTION, GUARD_PAGE_PROTECTION};
use crate::{
    libc::mem::{mmap, mprotect, munmap},
    linked_list::{LinkedList, LinkedListNode},
};

pub struct MetadataAllocator<T> {
    partial_regions: LinkedList<RegionHeader<T>>,
    full_regions: LinkedList<RegionHeader<T>>,
    empty_region: *mut LinkedListNode<RegionHeader<T>>,
    page_size: usize,
    slots_per_region: u32,
    slots_offset: usize,
    _marker: PhantomData<T>,
}

impl<T> MetadataAllocator<T> {
    pub fn new(page_size: usize) -> Self {
        const {
            assert!(
                size_of::<T>() > 0,
                "MetadataAllocator does not support zero-sized types"
            );
        }

        debug_assert!(page_size.is_power_of_two());

        let (slots_per_region, slots_offset) = RegionHeader::<T>::compute_slot_geometry(page_size);

        Self {
            partial_regions: LinkedList::new(),
            full_regions: LinkedList::new(),
            empty_region: null_mut(),
            page_size,
            slots_per_region,
            slots_offset,
            _marker: PhantomData,
        }
    }

    pub fn alloc(&mut self) -> *mut T {
        unsafe {
            if self.partial_regions.is_empty() {
                self.activate_region();
            }

            let region = self.partial_regions.front();
            let bitmap = RegionHeader::<T>::bitmap_start(region);

            let slot_index = RegionHeader::<T>::find_free_slot(
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

            (*region).value.slots_occupied += 1;

            if (*region).value.slots_occupied == self.slots_per_region {
                (*region).list_remove();
                self.full_regions.list_push_front(region);
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
            let bitmap = RegionHeader::<T>::bitmap_start(region);

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

            let was_full = (*region).value.slots_occupied == self.slots_per_region;
            (*region).value.slots_occupied -= 1;

            if was_full {
                (*region).list_remove();
                if (*region).value.slots_occupied == 0 {
                    self.cache_or_destroy_region(region);
                } else {
                    self.partial_regions.list_push_front(region);
                }
            } else if (*region).value.slots_occupied == 0 {
                (*region).list_remove();
                self.cache_or_destroy_region(region);
            }
        }
    }

    unsafe fn activate_region(&mut self) {
        if !self.empty_region.is_null() {
            let empty = self.empty_region;
            self.empty_region = null_mut();

            #[cfg(debug_assertions)]
            {
                let bitmap = RegionHeader::<T>::bitmap_start(empty);
                let bitmap_bytes = ((self.slots_per_region + 7) / 8) as usize;
                assert!(
                    (0..bitmap_bytes).all(|index| *bitmap.add(index) == 0),
                    "cached empty region has non-zero bitmap"
                );
            }

            self.partial_regions.list_push_front(empty);
        } else {
            self.create_region();
        }
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

        let region = usable_page as *mut LinkedListNode<RegionHeader<T>>;
        // The bitmap bytes are guaranteed zero by the kernel (anonymous mmap pages are always zeroed),
        // which represents all slots free.
        ptr::write(region, LinkedListNode::new(RegionHeader::new()));

        self.partial_regions.list_push_front(region);
    }

    // SAFETY: The region must have been removed from its list before calling this.
    unsafe fn cache_or_destroy_region(&mut self, region: *mut LinkedListNode<RegionHeader<T>>) {
        if self.empty_region.is_null() {
            self.empty_region = region;
        } else {
            self.destroy_region(region);
        }
    }

    unsafe fn destroy_region(&self, region: *mut LinkedListNode<RegionHeader<T>>) {
        let mmap_start = (region as *mut u8).sub(self.page_size);
        let total_size = self.page_size * 3;
        munmap(mmap_start, total_size);
    }

    // Masking off the low bits of a slot pointer recovers the region header
    // because each region fits within a single page-aligned page.
    unsafe fn region_from_pointer(&self, pointer: *mut T) -> *mut LinkedListNode<RegionHeader<T>> {
        let region_mask = !(self.page_size - 1);
        (pointer as usize & region_mask) as *mut LinkedListNode<RegionHeader<T>>
    }
}
