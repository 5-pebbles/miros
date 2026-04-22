use core::ptr;
use std::{alloc::Layout, ptr::null_mut};

use crate::{
    allocator::{
        metadata_allocator::MetadataAllocator, ANONYMOUS_PRIVATE_MAP, DATA_PAGE_PROTECTION,
    },
    libc::mem::{mmap, munmap},
    linked_list::{LinkedList, LinkedListNode},
    page_size,
};

const CAPACITY: usize = 8;
const MAX_ALIGNMENT: usize = 128 * 1024;

type EntryIndex = u8;
const _: () = assert!(CAPACITY <= EntryIndex::MAX as usize);

pub struct LargeAllocator {
    metadata: MetadataAllocator<LinkedListNode<LargeRegion>>,
    allocations: LinkedList<LargeRegion>,
    cache: LargeCache,
}

impl LargeAllocator {
    pub fn new() -> Self {
        Self {
            metadata: MetadataAllocator::new(),
            allocations: LinkedList::new(),
            cache: LargeCache::new(),
        }
    }

    pub fn alloc_large(&mut self, layout: Layout) -> *mut u8 {
        if layout.align() > MAX_ALIGNMENT {
            return null_mut();
        }

        let total_bytes = page_size::get_page_end(layout.size());

        let region = self.cache.take(total_bytes).unwrap_or_else(|| {
            let fresh = unsafe {
                mmap(
                    null_mut(),
                    total_bytes,
                    DATA_PAGE_PROTECTION,
                    ANONYMOUS_PRIVATE_MAP,
                    -1, /* file_descriptor */
                    0,  /* file_offset */
                )
            };
            LargeRegion {
                pointer: fresh,
                size_in_bytes: total_bytes,
            }
        });

        let record = self.metadata.alloc();
        unsafe {
            ptr::write(record, LinkedListNode::new(region));
            self.allocations.list_push_front(record);
        }

        region.pointer
    }

    pub fn dealloc_large(&mut self, pointer: *mut u8) {
        unsafe {
            let node = self.region_from_ptr(pointer);

            let region = (*node).value;
            (*node).list_remove();
            self.metadata.dealloc(node);

            if !self.cache.park(region) {
                munmap(region.pointer, region.size_in_bytes);
            }
        }
    }

    /// Look up the mapped size of a live large allocation.
    pub fn allocation_size(&self, pointer: *mut u8) -> usize {
        unsafe { (*self.region_from_ptr(pointer)).value.size_in_bytes }
    }

    fn region_from_ptr(&self, pointer: *mut u8) -> *mut LinkedListNode<LargeRegion> {
        unsafe {
            self.allocations
                .iter()
                .find(|&node| (*node).value.pointer == pointer)
                .unwrap_unchecked()
        }
    }
}

#[derive(Clone, Copy)]
pub struct LargeRegion {
    pub pointer: *mut u8,
    pub size_in_bytes: usize,
}

pub struct LargeCache {
    entries: [LargeRegion; CAPACITY],
    entry_count: EntryIndex,
}

impl LargeCache {
    pub const fn new() -> Self {
        Self {
            entries: [LargeRegion {
                pointer: ptr::null_mut(),
                size_in_bytes: 0,
            }; CAPACITY],
            entry_count: 0,
        }
    }

    /// Reclaim the tightest-fitting cached region with at least `minimum_bytes`.
    pub fn take(&mut self, minimum_bytes: usize) -> Option<LargeRegion> {
        let (index, _) = self.entries[..self.entry_count as usize]
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.size_in_bytes >= minimum_bytes)
            .min_by_key(|(_, entry)| entry.size_in_bytes)?;

        let region = self.entries[index];
        self.entry_count -= 1;
        self.entries[index] = self.entries[self.entry_count as usize];
        Some(region)
    }

    /// Attempt to cache a freed region for reuse. Returns `true` if stored,
    /// `false` if the cache is full and the caller should unmap.
    pub fn park(&mut self, region: LargeRegion) -> bool {
        if (self.entry_count as usize) >= CAPACITY {
            return false;
        }

        self.entries[self.entry_count as usize] = region;
        self.entry_count += 1;
        true
    }
}
