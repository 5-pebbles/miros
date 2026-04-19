use std::{ffi::c_void, ptr::null_mut};

use crate::allocator::primary_allocator::span::BaseSpan;

const CAPACITY: usize = 16;

type EntryIndex = u8;
const _: () = assert!(CAPACITY <= EntryIndex::MAX as usize);

#[derive(Clone, Copy)]
pub struct RecentFreeEntry {
    span: *mut BaseSpan,
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

pub struct RecentFreeStack {
    entries: [RecentFreeEntry; CAPACITY],
    entry_count: EntryIndex,
}

impl RecentFreeStack {
    const fn new() -> Self {
        Self {
            entries: [const { RecentFreeEntry::empty() }; CAPACITY],
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
        if self.entry_count as usize >= CAPACITY {
            return false;
        }

        self.entries[self.entry_count as usize] = entry;
        self.entry_count += 1;
        true
    }
}
