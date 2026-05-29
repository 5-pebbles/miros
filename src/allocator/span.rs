use core::sync::atomic::{AtomicU64, Ordering};

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
    /// Cross-thread frees `fetch_or` their slot here; the owner drains it into `bitmap`.
    remote_freed: [AtomicU64; BITMAP_WORD_COUNT],
    /// Bit N = 1 iff `remote_freed[N]` may hold a pending free.
    remote_summary: AtomicU64,
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

        // Mark the partially-filled final word as having free slots.
        // Only set when the slot count doesn't fill the last word exactly.
        // An exact multiple has no partial word, and skipping the branch keeps the shift below the word width.
        let partial_bit = if trailing_bits > 0 {
            (1 as BitmapWord) << full_words
        } else {
            0
        };

        Self {
            data_pointer,
            size_class,
            slots_occupied: 0,
            summary: full_word_mask | partial_bit,
            bitmap,
            remote_freed: core::array::from_fn(|_| AtomicU64::new(0)),
            remote_summary: AtomicU64::new(0),
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

    /// Wait-free free from a non-owning thread; touches only the atomic side channel.
    pub fn remote_dealloc_slot(&self, pointer: *const u8) {
        let slot_index = self.slot_index_of(pointer);
        let (word, bit) = word_and_bit(slot_index);

        self.remote_freed[word].fetch_or((1 as BitmapWord) << bit, Ordering::Release);
        // Summary after the word: an observed summary bit implies the word is visible.
        self.remote_summary
            .fetch_or((1 as BitmapWord) << word, Ordering::Release);
    }

    /// Owner-side drain of pending remote frees into `bitmap`; returns the count.
    pub fn reclaim_remote_frees(&mut self) -> usize {
        let mut pending_words = self.remote_summary.swap(0, Ordering::Acquire);
        let mut reclaimed = 0;

        while pending_words != 0 {
            let word = pending_words.trailing_zeros() as usize;
            pending_words &= pending_words - 1;

            let freed = self.remote_freed[word].swap(0, Ordering::Acquire);
            if freed == 0 {
                continue;
            }

            // Double/wild free: a remote bit set where `bitmap` shows the slot free.
            assert!(
                freed & !self.bitmap[word] == 0,
                "remote free of an unallocated slot (double-free or wild-free)"
            );
            self.bitmap[word] &= !freed;
            self.summary |= (1 as BitmapWord) << word;
            reclaimed += freed.count_ones() as usize;
        }

        self.slots_occupied -= reclaimed as SlotIndex;
        reclaimed
    }

    pub fn has_remote_frees(&self) -> bool {
        self.remote_summary.load(Ordering::Relaxed) != 0
    }

    pub fn is_full(&self) -> bool {
        self.slots_occupied == self.slots_per_span()
    }

    pub fn is_empty(&self) -> bool {
        self.slots_occupied == 0
    }

    /// Reset owner-side occupancy only — `*self = Self::new(..)` would clobber the remote atomics.
    pub fn reinitialize(&mut self) {
        let fresh = Self::new(self.data_pointer, self.size_class);
        self.slots_occupied = fresh.slots_occupied;
        self.summary = fresh.summary;
        self.bitmap = fresh.bitmap;
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

#[cfg(test)]
mod tests {
    use super::*;

    struct SpanFixture {
        span: Span,
        _backing: Vec<u8>,
    }

    fn fixture() -> SpanFixture {
        let size_class = SizeClass::from_raw(0);
        let mut backing = vec![0u8; size_class.span_length_in_bytes()];
        let span = Span::new(backing.as_mut_ptr(), size_class);
        SpanFixture {
            span,
            _backing: backing,
        }
    }

    /// `random == 0` is deterministic: falls through to `trailing_zeros`, lowest slot.
    fn alloc(span: &mut Span) -> *mut u8 {
        span.allocate_slot(0).expect("span has free slots")
    }

    #[test]
    fn remote_free_is_invisible_until_reclaimed() {
        let SpanFixture { mut span, _backing } = fixture();

        let first = alloc(&mut span);
        let _second = alloc(&mut span);
        let third = alloc(&mut span);
        assert_eq!(span.slots_occupied, 3);

        span.remote_dealloc_slot(first);
        span.remote_dealloc_slot(third);

        assert_eq!(span.slots_occupied, 3);
        assert!(span.has_remote_frees());

        let reclaimed = span.reclaim_remote_frees();
        assert_eq!(reclaimed, 2);
        assert_eq!(span.slots_occupied, 1);
        assert!(!span.has_remote_frees());
    }

    #[test]
    fn reclaimed_slots_are_handed_out_again() {
        let SpanFixture { mut span, _backing } = fixture();

        let only = alloc(&mut span);
        span.remote_dealloc_slot(only);
        assert_eq!(span.reclaim_remote_frees(), 1);
        assert_eq!(span.slots_occupied, 0);

        let reused = alloc(&mut span);
        assert_eq!(reused, only);
        assert_eq!(span.slots_occupied, 1);
    }

    #[test]
    fn reclaim_with_no_remote_frees_is_a_noop() {
        let SpanFixture { mut span, _backing } = fixture();
        alloc(&mut span);
        assert_eq!(span.reclaim_remote_frees(), 0);
        assert_eq!(span.slots_occupied, 1);
    }

    #[test]
    fn remote_frees_spanning_multiple_words_all_reclaim() {
        let SpanFixture { mut span, _backing } = fixture();

        let pointers: Vec<*mut u8> = (0..200).map(|_| alloc(&mut span)).collect();
        assert_eq!(span.slots_occupied, 200);

        for pointer in &pointers {
            span.remote_dealloc_slot(*pointer);
        }
        assert_eq!(span.reclaim_remote_frees(), 200);
        assert_eq!(span.slots_occupied, 0);
    }
}
