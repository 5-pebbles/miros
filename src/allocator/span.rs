use super::size_classes::SizeClass;

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

pub struct Span {
    data_pointer: *mut u8,
    size_class: SizeClass,
    slots_occupied: SlotIndex,
    /// Bit N = 1 iff `bitmap[N]` has a free slot.
    summary: BitmapWord,
    /// Bits beyond `slots_per_span` are pre-set to 1 so scans don't need range checks.
    bitmap: [BitmapWord; BITMAP_WORD_COUNT],
}

impl Span {
    pub fn new(data_pointer: *mut u8, size_class: SizeClass) -> Self {
        let slots_total = size_class.slots_per_span() as SlotIndex;
        debug_assert!(slots_total > 0 && slots_total as usize <= MAX_SLOTS_PER_SPAN);

        let (full_words, trailing_bits) = word_and_bit(slots_total);

        let mut bitmap = [0; BITMAP_WORD_COUNT];
        bitmap[full_words..BITMAP_WORD_COUNT]
            .iter_mut()
            .for_each(|word| *word = BitmapWord::MAX);
        bitmap
            .get_mut(full_words)
            .map(|word| *word &= BitmapWord::MAX.wrapping_shl(trailing_bits as u32));

        let full_word_mask = if full_words >= BITMAP_WORD_COUNT {
            BitmapWord::MAX
        } else {
            (1 as BitmapWord)
                .wrapping_shl(full_words as u32)
                .wrapping_sub(1)
        };
        let partial_bit = (trailing_bits > 0)
            .then_some((1 as BitmapWord) << full_words)
            .unwrap_or(0);

        Self {
            data_pointer,
            size_class,
            slots_occupied: 0,
            summary: full_word_mask | partial_bit,
            bitmap,
        }
    }

    pub fn allocate_slot(&mut self, random: u64) -> Option<*mut u8> {
        if self.summary == 0 {
            return None;
        }

        // Random rotation spreads allocations across words.
        let rotation = (random >> 16) as u32;
        let rotated = self.summary.rotate_right(rotation);
        let word_index =
            ((rotation + rotated.trailing_zeros()) % BITMAP_WORD_COUNT as u32) as usize;

        let free_bits = !self.bitmap[word_index];
        debug_assert!(free_bits != 0, "summary bit set but word is full");

        // Random mask scatters within the word; fall back to trailing_zeros.
        let within_word_mask = random | random.rotate_left(13);
        let masked = free_bits & within_word_mask;
        let bit_index = if masked != 0 {
            masked.trailing_zeros() as usize
        } else {
            free_bits.trailing_zeros() as usize
        };

        let slot_index = (word_index * BitmapWord::BITS as usize + bit_index) as SlotIndex;
        debug_assert!(slot_index < self.slots_per_span());
        debug_assert!(!self.is_occupied(slot_index));

        self.slots_occupied += 1;
        self.bitmap[word_index] |= (1 as BitmapWord) << bit_index;
        if self.bitmap[word_index] == BitmapWord::MAX {
            self.summary &= !((1 as BitmapWord) << word_index);
        }

        // SAFETY: slot_index < slots_per_span, so the offset is within the span's backing allocation.
        Some(unsafe {
            self.data_pointer
                .byte_add((slot_index as usize) << self.size_class.slot_shift())
        })
    }

    pub fn dealloc_slot(&mut self, pointer: *const u8) {
        let slot_index = self.slot_index_of(pointer);
        debug_assert!(self.is_occupied(slot_index));

        self.slots_occupied -= 1;
        let (word, bit) = word_and_bit(slot_index);
        self.bitmap[word] &= !((1 as BitmapWord) << bit);
        self.summary |= (1 as BitmapWord) << word;
    }

    pub fn is_full(&self) -> bool {
        self.slots_occupied == self.slots_per_span()
    }

    pub fn is_empty(&self) -> bool {
        self.slots_occupied == 0
    }

    pub fn reinitialize(&mut self) {
        *self = Self::new(self.data_pointer, self.size_class);
    }

    pub fn contains_pointer(&self, pointer: *const u8) -> bool {
        let pointer_address = pointer.addr();
        let self_data_address = self.data_pointer.addr();
        let span_length = self.size_class.span_length_in_bytes();
        (self_data_address..self_data_address + span_length).contains(&pointer_address)
    }

    /* Private */

    fn slots_per_span(&self) -> SlotIndex {
        self.size_class.slots_per_span() as SlotIndex
    }

    fn is_occupied(&self, slot_index: SlotIndex) -> bool {
        let (word, bit) = word_and_bit(slot_index);
        (self.bitmap[word] & ((1 as BitmapWord) << bit)) != 0
    }

    fn slot_index_of(&self, pointer: *const u8) -> SlotIndex {
        debug_assert!(self.contains_pointer(pointer));
        let pointer_delta = pointer.addr() - self.data_pointer.addr();
        let slot_index = (pointer_delta >> self.size_class.slot_shift()) as SlotIndex;
        debug_assert!(slot_index < self.slots_per_span());
        slot_index
    }
}
