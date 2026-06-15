mod occupancy;

use std::{cell::UnsafeCell, sync::atomic::Ordering};

use super::size_classes::SizeClass;
use crate::allocator::{
    heap::heap::HeapId,
    span::occupancy::{LocalOccupancy, RemoteOccupancy, SlotIndex},
};

// PERF: The remotes are separate because atomics are slow (10x), and I don't want to add that strain to the hotpath.
pub struct Span {
    data_pointer: *mut u8,
    size_class: SizeClass,
    owner: HeapId,
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
            owner,
            local: UnsafeCell::new(LocalOccupancy::new(size_class)),
            remote: RemoteOccupancy::new(),
        }
    }

    pub unsafe fn adopt(&self, owner: u64) {
        self.owner.store(owner, Ordering::Relaxed);
    }

    pub unsafe fn allocate_slot(&self, random: u64) -> Option<*mut u8> {
        unsafe {
            let slot_index = (*self.local.get()).claim_random_slot(random)?;

            // SAFETY: slot_index < slots_per_span, so the offset is within the span's backing allocation.
            Some(
                self.data_pointer
                    .byte_add((slot_index as usize) << self.size_class.slot_shift()),
            )
        }
    }

    pub unsafe fn dealloc_slot(&self, pointer: *const u8) {
        unsafe {
            let slot_index = self.slot_index_of(pointer);
            debug_assert!((*self.local.get()).occupancy.is_slot_occupied(slot_index));

            (*self.local.get()).release_slot(slot_index);
        }
    }

    pub unsafe fn remote_dealloc_slot(&self, pointer: *const u8) {
        unsafe {
            let slot_index = self.slot_index_of(pointer);

            self.remote.remote_dealloc_slot(slot_index)
        }
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
