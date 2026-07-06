mod occupancy;

use std::{cell::UnsafeCell, sync::atomic::Ordering};

pub use occupancy::{BitmapWord, MAX_SLOTS_PER_SPAN};

use super::size_classes::SizeClass;
use crate::allocator::{
    heap::heap::{AtomicHeapId, HeapId},
    span::occupancy::{LocalOccupancy, RemoteOccupancy, SlotIndex},
};

// PERF: The remotes are separate because atomics are slow (10x), and I don't want to add that strain to the hotpath.
pub struct Span {
    data_pointer: *mut u8,
    size_class: SizeClass,
    owner: AtomicHeapId,
    /// Bit N = 0 iff `bitmap[N]` has a free slot.
    /// Bits beyond `slots_per_span` are pre-set to 1 so scans don't need range checks.
    local: UnsafeCell<LocalOccupancy>,
    /// Bit N = 1 iff `remote_freed[N]` may hold a pending free.
    /// Cross-thread frees `fetch_or` their slot here; the owner drains it into `bitmap`.
    remote: RemoteOccupancy,
}

impl Span {
    pub unsafe fn new(data_pointer: *mut u8, size_class: SizeClass, owner: HeapId) -> Self {
        Self {
            data_pointer,
            size_class,
            owner: AtomicHeapId::new(owner),
            local: UnsafeCell::new(LocalOccupancy::new(size_class)),
            remote: RemoteOccupancy::new(),
        }
    }

    pub unsafe fn set_owner(&self, owner: HeapId) {
        self.owner.store(owner, Ordering::Relaxed);
    }

    /// A routing hint only — the remote bitmap's `Release`/`Acquire` carries the cross-thread ordering, not this.
    pub fn owner(&self) -> HeapId {
        self.owner.load(Ordering::Relaxed)
    }

    pub fn is_full(&self) -> bool {
        unsafe { (*self.local.get()).slots_occupied == self.slots_per_span() }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { (*self.local.get()).slots_occupied == 0 }
    }

    pub fn has_remote_frees(&self) -> bool {
        self.remote.has_remote_frees()
    }

    /// Reset owner-side occupancy only —
    /// overwriting `remote` would clobber the atomics a non-owner may be writing this instant.
    pub fn reinitialize(&self) {
        unsafe { *self.local.get() = LocalOccupancy::new(self.size_class) }
    }

    /// Claim up to `max` free slots from a random word for a magazine refill; `random` scatters them across the span.
    #[inline(always)]
    pub unsafe fn claim_up_to(&self, max: u32, random: u64) -> Option<ClaimedSlots> {
        let (word_index, mask) = (*self.local.get()).claim_up_to(max, random)?;

        // SAFETY: word_index < BITMAP_WORD_COUNT, so the word's first slot is within the span.
        let base = self
            .data_pointer
            .byte_add((word_index * BitmapWord::BITS as usize) << self.size_class.slot_shift());
        Some(ClaimedSlots {
            base,
            slot_shift: self.size_class.slot_shift(),
            mask,
        })
    }

    pub unsafe fn dealloc_slot(&self, pointer: *const u8) {
        let slot_index = self.slot_index_of(pointer);
        debug_assert!((*self.local.get()).occupancy.is_slot_occupied(slot_index));

        (*self.local.get()).release_slot(slot_index);
    }

    pub unsafe fn remote_dealloc_slot(&self, pointer: *const u8) {
        let slot_index = self.slot_index_of(pointer);

        self.remote.remote_dealloc_slot(slot_index)
    }

    pub fn reclaim_remote_frees(&self) {
        self.remote
            .iter_reclaim_remote_free_words()
            .for_each(|(word_index, freed)| unsafe {
                (*self.local.get()).release_slots_by_word(word_index, freed)
            });
    }

    pub fn contains_pointer(&self, pointer: *const u8) -> bool {
        let pointer_address = pointer.addr();
        let self_data_address = self.data_pointer.addr();
        let span_length = self.size_class.span_length_in_bytes();
        (self_data_address..self_data_address + span_length).contains(&pointer_address)
    }

    fn slot_index_of(&self, pointer: *const u8) -> SlotIndex {
        debug_assert!(self.contains_pointer(pointer));
        let pointer_delta = pointer.addr() - self.data_pointer.addr();
        let slot_index = (pointer_delta >> self.size_class.slot_shift()) as SlotIndex;
        debug_assert!(slot_index < self.slots_per_span());
        slot_index
    }

    fn slots_per_span(&self) -> SlotIndex {
        self.size_class.slots_per_span() as SlotIndex
    }
}

/// Expands a claimed word into pointers, keeping the bitmap's bit polarity sealed in the span.
pub struct ClaimedSlots {
    base: *mut u8,
    slot_shift: u32,
    mask: BitmapWord,
}

impl Iterator for ClaimedSlots {
    type Item = *mut u8;

    #[inline(always)]
    fn next(&mut self) -> Option<*mut u8> {
        (self.mask != 0).then(|| {
            let offset_into_word = self.mask.trailing_zeros();
            self.mask &= self.mask - 1;
            // SAFETY: offset_into_word < 64 and the word lies within the span's backing allocation.
            unsafe {
                self.base
                    .byte_add((offset_into_word as usize) << self.slot_shift)
            }
        })
    }
}
