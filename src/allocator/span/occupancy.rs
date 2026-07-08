use std::{
    arch::x86_64::_pdep_u64,
    iter::from_fn,
    ops::Deref,
    sync::atomic::{Atomic, Ordering},
};

use crate::allocator::size_classes::SizeClass;

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

pub struct Occupancy<T> {
    summary: T,
    bitmap: [T; BITMAP_WORD_COUNT],
}

impl Occupancy<BitmapWord> {
    pub fn new(size_class: SizeClass) -> Self {
        let slots_total = size_class.slots_per_span() as SlotIndex;
        debug_assert!(slots_total > 0 && slots_total as usize <= MAX_SLOTS_PER_SPAN);

        let (full_words, trailing_bits) = word_and_bit(slots_total);

        let mut bitmap = [0; BITMAP_WORD_COUNT];
        // Out-of-range words pre-marked full; the partial word keeps its low slots free.
        bitmap[full_words..BITMAP_WORD_COUNT]
            .iter_mut()
            .for_each(|word| *word = BitmapWord::MAX);
        bitmap
            .get_mut(full_words)
            .map(|word| *word = BitmapWord::MAX << trailing_bits);

        // `checked_shl` yields 0 when slots fill every word exactly (live_words == 64).
        let live_words = full_words + (trailing_bits > 0) as usize;
        let summary = BitmapWord::MAX.checked_shl(live_words as u32).unwrap_or(0);

        Self { summary, bitmap }
    }

    /// Claim up to `max` free slots from a *random* free word, taking a random window of its bits.
    /// Randomizing the claim (not just the draw) keeps placement unpredictable for big classes, whose span is a single short word.
    /// Returns the word index and a mask of claimed slots (set bit = claimed).
    pub fn claim_up_to(&mut self, max: u32, random: u64) -> Option<(usize, BitmapWord)> {
        if self.summary == BitmapWord::MAX || max == 0 {
            return None;
        }

        // Random free word: free words are the zero bits of summary; rotate the scan start.
        let free_words = !self.summary;
        let word_rotation = (random & 63) as u32;
        let word_index = (word_rotation
            .wrapping_add(free_words.rotate_right(word_rotation).trailing_zeros())
            & 63) as usize;

        let free = !self.bitmap[word_index];
        debug_assert_ne!(free, 0, "summary clear but word is full");

        // Whole word if it fits in `max`; else a random window: rotate, take the lowest `max` set bits (PDEP), rotate back.
        let claimed = if max >= free.count_ones() {
            free
        } else {
            let bit_rotation = ((random >> 6) & 63) as u32;
            let rotated = free.rotate_right(bit_rotation);
            let window = unsafe { _pdep_u64((1 << max) - 1, rotated) };
            window.rotate_left(bit_rotation)
        };

        self.bitmap[word_index] |= claimed;
        if self.bitmap[word_index] == BitmapWord::MAX {
            self.summary |= (1 as BitmapWord) << word_index;
        }
        Some((word_index, claimed))
    }

    pub fn release_slot(&mut self, slot_index: SlotIndex) {
        let (word, bit) = word_and_bit(slot_index);
        self.bitmap[word] &= !((1 as BitmapWord) << bit);
        self.summary &= !((1 as BitmapWord) << word);
    }

    pub fn release_slots_by_word(&mut self, word_index: usize, freed: BitmapWord) {
        if freed == 0 {
            return;
        }
        assert!(
            freed & !self.bitmap[word_index] == 0,
            "remote free of an unallocated slot (double-free or wild-free)"
        );
        self.bitmap[word_index] &= !freed;
        self.summary &= !(1 << word_index);
    }

    pub fn is_slot_occupied(&self, slot_index: SlotIndex) -> bool {
        let (word, bit) = word_and_bit(slot_index);
        self.bitmap[word] & ((1 as BitmapWord) << bit) != 0
    }
}

impl Occupancy<Atomic<BitmapWord>> {
    pub fn atomic_zeroed() -> Self {
        Self {
            summary: Atomic::<BitmapWord>::new(0),
            bitmap: [const { Atomic::<BitmapWord>::new(0) }; BITMAP_WORD_COUNT],
        }
    }

    /// Wait-free free from a non-owning thread; touches only the atomic side channel.
    pub unsafe fn remote_dealloc_slot(&self, slot_index: SlotIndex) {
        let (word, bit) = word_and_bit(slot_index);

        self.bitmap[word].fetch_or((1 as BitmapWord) << bit, Ordering::Release);
        // Summary written after the word so an observed summary bit implies the word is visible.
        self.summary
            .fetch_or((1 as BitmapWord) << word, Ordering::Release);
    }

    pub fn has_remote_frees(&self) -> bool {
        self.summary.load(Ordering::Relaxed) != 0
    }

    pub fn iter_reclaim_remote_free_words(&self) -> impl Iterator<Item = (usize, BitmapWord)> + '_ {
        let mut pending_words = self.summary.swap(0, Ordering::Acquire);
        from_fn(move || {
            if pending_words == 0 {
                return None;
            }
            let word_index = pending_words.trailing_zeros() as usize;
            pending_words &= pending_words - 1;

            let word = self.bitmap[word_index].swap(0, Ordering::Acquire);
            Some((word_index, word))
        })
    }
}

#[repr(align(64))]
pub struct LocalOccupancy {
    pub slots_occupied: SlotIndex,
    pub occupancy: Occupancy<BitmapWord>,
}

impl LocalOccupancy {
    pub fn new(size_class: SizeClass) -> Self {
        Self {
            slots_occupied: 0,
            occupancy: Occupancy::new(size_class),
        }
    }

    pub fn claim_up_to(&mut self, max: u32, random: u64) -> Option<(usize, BitmapWord)> {
        let (word_index, claimed) = self.occupancy.claim_up_to(max, random)?;
        self.slots_occupied += claimed.count_ones() as u16;
        Some((word_index, claimed))
    }

    pub fn release_slot(&mut self, slot_index: SlotIndex) {
        self.slots_occupied -= 1;
        self.occupancy.release_slot(slot_index);
    }

    pub fn release_slots_by_word(&mut self, word_index: usize, freed: BitmapWord) {
        self.occupancy.release_slots_by_word(word_index, freed);
        self.slots_occupied -= freed.count_ones() as u16;
    }
}

#[repr(align(64))]
pub struct RemoteOccupancy {
    pub occupancy: Occupancy<Atomic<BitmapWord>>,
}

impl RemoteOccupancy {
    pub fn new() -> Self {
        Self {
            occupancy: Occupancy::atomic_zeroed(),
        }
    }
}

impl Deref for RemoteOccupancy {
    type Target = Occupancy<Atomic<BitmapWord>>;

    fn deref(&self) -> &Self::Target {
        &self.occupancy
    }
}
