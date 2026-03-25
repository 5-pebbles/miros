use std::{marker::PhantomData, mem::size_of, ptr::null_mut};

// Region layout: [Header][Bitmap bytes][align pad][Slot 0][Slot 1]...[Slot N-1]
#[repr(C)]
pub(super) struct RegionHeader<T> {
    next: *mut Self,
    prev: *mut Self,
    pub(super) slots_occupied: u32,
    _marker: PhantomData<T>,
}

impl<T> RegionHeader<T> {
    pub(super) fn new_zeroed() -> Self {
        Self {
            next: null_mut(),
            prev: null_mut(),
            slots_occupied: 0,
            _marker: PhantomData,
        }
    }

    pub(super) fn compute_slot_geometry(page_size: usize) -> (u32, usize) {
        let header_size = size_of::<Self>();
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

    pub(super) unsafe fn bitmap_start(region: *mut Self) -> *mut u8 {
        (region as *mut u8).add(size_of::<Self>())
    }

    pub(super) unsafe fn find_free_slot(
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

    pub(super) unsafe fn list_push_front(head: &mut *mut Self, node: *mut Self) {
        let old_head = *head;
        (*node).next = old_head;
        (*node).prev = null_mut();
        if let Some(old_head) = old_head.as_mut() {
            old_head.prev = node;
        }
        *head = node;
    }

    pub(super) unsafe fn list_remove(head: &mut *mut Self, node: *mut Self) {
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
}
