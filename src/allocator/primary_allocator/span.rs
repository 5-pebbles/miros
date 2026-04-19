use std::{ffi::c_void, ptr::null_mut};

use crate::allocator::primary_allocator::size_classes::SizeClass;

pub type BitmapWord = u64;
pub const BITMAP_WORD_COUNT: usize = 64;

pub const MAX_SLOTS_PER_SPAN: usize = BITMAP_WORD_COUNT * BitmapWord::BITS as usize;

pub type SlotIndex = u16;
const _: () = assert!(MAX_SLOTS_PER_SPAN <= SlotIndex::MAX as usize);

fn word_and_bit(slot_index: u16) -> (usize, usize) {
    (
        slot_index as usize / BitmapWord::BITS as usize,
        slot_index as usize % BitmapWord::BITS as usize,
    )
}

#[repr(C)]
pub struct BaseSpan {
    next: *mut Self,
    prev: *mut Self,
    data_pointer: *mut c_void,
    size_class: SizeClass,
    slots_total: SlotIndex,
    slots_occupied: SlotIndex,
    /// Bits beyond `slots_total` are pre-set to 1 so hot path doesn't need range checks on bitmap scans.
    bitmap: [BitmapWord; BITMAP_WORD_COUNT],
}

impl BaseSpan {
    pub fn new(data_pointer: *mut c_void, slots_total: SlotIndex, size_class: SizeClass) -> Self {
        debug_assert!(slots_total > 0 && slots_total as usize <= MAX_SLOTS_PER_SPAN);

        let (full_words, trailing_bits) = word_and_bit(slots_total);

        let mut bitmap = [0; BITMAP_WORD_COUNT];
        bitmap[full_words..BITMAP_WORD_COUNT]
            .iter_mut()
            .for_each(|word| *word = BitmapWord::MAX);
        bitmap
            .get_mut(full_words)
            .map(|word| *word &= BitmapWord::MAX.wrapping_shl(trailing_bits as u32));

        Self {
            next: null_mut(),
            prev: null_mut(),
            data_pointer,
            size_class,
            slots_total,
            slots_occupied: 0,
            bitmap,
        }
    }

    pub fn allocate_slot(&mut self, random: u64) -> Option<*mut c_void> {
        let active_words = (self.slots_total as usize).div_ceil(BitmapWord::BITS as usize);
        debug_assert!(active_words != 0);

        let start_word = (random as usize) % active_words;

        (0..active_words).find_map(|offset| {
            let word_index = (start_word + offset) % active_words;
            let free_bits = !self.bitmap[word_index];
            (free_bits != 0).then(|| {
                let slot_index = (word_index * BitmapWord::BITS as usize
                    + free_bits.trailing_zeros() as usize)
                    as SlotIndex;
                debug_assert!(slot_index < self.slots_total);
                debug_assert!(!self.is_occupied(slot_index));

                self.slots_occupied += 1;
                let (word, bit) = word_and_bit(slot_index);
                self.bitmap[word] |= (1 as BitmapWord) << bit;

                // SAFETY: slot_index < slots_total, so the offset is within the span's backing allocation.
                unsafe {
                    self.data_pointer
                        .byte_add((slot_index as usize) << self.size_class.slot_shift())
                }
            })
        })
    }

    pub fn release_slot(&mut self, pointer: *const c_void) {
        let slot_index = self.slot_index_of(pointer);
        debug_assert!(self.is_occupied(slot_index));

        self.slots_occupied -= 1;
        let (word, bit) = word_and_bit(slot_index);
        self.bitmap[word] &= !((1 as BitmapWord) << bit);
    }

    pub fn is_full(&self) -> bool {
        self.slots_occupied == self.slots_total
    }

    pub fn is_empty(&self) -> bool {
        self.slots_occupied == 0
    }

    pub fn contains_pointer(&self, pointer: *const c_void) -> bool {
        let pointer_address = pointer.addr();
        let self_data_address = self.data_pointer.addr();
        let span_length = self.size_class.span_length_in_bytes();
        (self_data_address..self_data_address + span_length).contains(&pointer_address)
    }

    /* Private */

    fn is_occupied(&self, slot_index: SlotIndex) -> bool {
        let (word, bit) = word_and_bit(slot_index);
        (self.bitmap[word] & ((1 as BitmapWord) << bit)) != 0
    }

    fn slot_index_of(&self, pointer: *const c_void) -> SlotIndex {
        debug_assert!(self.contains_pointer(pointer));
        let pointer_delta = pointer.addr() - self.data_pointer.addr();
        let slot_index = (pointer_delta >> self.size_class.slot_shift()) as SlotIndex;
        debug_assert!(slot_index < self.slots_total);
        slot_index
    }
}
