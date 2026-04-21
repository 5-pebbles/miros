use std::{ffi::c_void, ptr::null_mut};

use crate::{
    allocator::primary_allocator::span::Span,
    linked_list::{LinkedList, LinkedListNode},
};

const RECENT_FREE_CAPACITY: usize = 16;

type EntryIndex = u8;
const _: () = assert!(RECENT_FREE_CAPACITY <= EntryIndex::MAX as usize);

#[derive(Clone, Copy)]
struct RecentFreeEntry {
    span: *mut LinkedListNode<Span>,
    slot_pointer: *mut c_void,
}

impl RecentFreeEntry {
    const fn empty() -> Self {
        Self {
            span: null_mut(),
            slot_pointer: null_mut(),
        }
    }
}

struct RecentFreeStack {
    entries: [RecentFreeEntry; RECENT_FREE_CAPACITY],
    entry_count: EntryIndex,
}

impl RecentFreeStack {
    const fn new() -> Self {
        Self {
            entries: [const { RecentFreeEntry::empty() }; RECENT_FREE_CAPACITY],
            entry_count: 0,
        }
    }

    fn pop(&mut self) -> Option<RecentFreeEntry> {
        (self.entry_count != 0).then(|| {
            self.entry_count -= 1;
            self.entries[self.entry_count as usize]
        })
    }

    fn try_push(&mut self, entry: RecentFreeEntry) -> bool {
        if self.entry_count as usize >= RECENT_FREE_CAPACITY {
            return false;
        }

        self.entries[self.entry_count as usize] = entry;
        self.entry_count += 1;
        true
    }
}

/// Per-size-class state managing a contiguous virtual address region subdivided into spans.
pub struct ClassRegion {
    base: *mut c_void,
    /// Each span occupies 1 << span_stride_shift bytes
    span_stride_shift: u32,
    next_span_offset: usize,
    partial_spans: LinkedList<Span>,
    full_spans: LinkedList<Span>,
    empty_spans: LinkedList<Span>,
    recent_free: RecentFreeStack,
}
