use core::ptr;
use std::{
    alloc::Layout,
    ptr::{null_mut, NonNull},
};

use super::{ANONYMOUS_PRIVATE_MAP, DATA_PAGE_PROTECTION};
use crate::{
    libc::mem::{mmap, munmap},
    page_size,
    utils::{
        linked_list::{LinkedList, LinkedListNode},
        metadata_allocator::MetadataAllocator,
    },
};

const CAPACITY: usize = 8;

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

    #[cold]
    pub fn alloc_large(&mut self, layout: Layout) -> Option<NonNull<u8>> {
        // Size 0 still yields a unique freeable block, matching the small path's class-0 slot.
        let total_bytes = page_size::get_page_end(layout.size().max(1));
        let region = self.acquire_region(total_bytes, layout.align())?;
        Some(self.register_allocation(region))
    }

    pub fn alloc_large_zeroed(&mut self, layout: Layout) -> Option<NonNull<u8>> {
        let total_bytes = page_size::get_page_end(layout.size().max(1));
        let mut region = self.acquire_region(total_bytes, layout.align())?;
        if !region.zeroed {
            unsafe {
                ptr::write_bytes(region.pointer, 0, region.size_in_bytes);
            }
        }
        region.zeroed = true;
        Some(self.register_allocation(region))
    }

    /// `None` when the kernel refuses the mapping — the error must surface as a clean null, never `MAP_FAILED`.
    fn acquire_region(&mut self, total_bytes: usize, alignment: usize) -> Option<LargeRegion> {
        let page_size = page_size::get_page_size();
        // Cached regions only guarantee page alignment.
        if alignment <= page_size {
            if let Some(region) = self.cache.take(total_bytes) {
                return Some(region);
            }
        }

        // Page-aligned mmap results sit at most `alignment - page_size` short of the next boundary.
        let slack = alignment.saturating_sub(page_size);
        let mapped_bytes = total_bytes.checked_add(slack)?;

        let pointer = unsafe {
            mmap(
                null_mut(),
                mapped_bytes,
                DATA_PAGE_PROTECTION,
                ANONYMOUS_PRIVATE_MAP,
                -1, /* file_descriptor */
                0,  /* file_offset */
            )
        };
        // The kernel returns `-errno` (a small negative) on failure; a valid mapping is always a positive address.
        if (pointer as isize) <= 0 {
            return None;
        }

        let raw_address = pointer.addr();
        let aligned_address = (raw_address + alignment - 1) & !(alignment - 1);
        let leading_slack = aligned_address - raw_address;
        let trailing_slack = mapped_bytes - leading_slack - total_bytes;
        unsafe {
            if leading_slack > 0 {
                munmap(pointer, leading_slack);
            }
            if trailing_slack > 0 {
                munmap(pointer.add(leading_slack + total_bytes), trailing_slack);
            }
        }

        Some(LargeRegion {
            pointer: unsafe { pointer.add(leading_slack) },
            size_in_bytes: total_bytes,
            zeroed: true,
        })
    }

    fn register_allocation(&mut self, region: LargeRegion) -> NonNull<u8> {
        let record = self.metadata.alloc();
        unsafe {
            ptr::write(record.as_ptr(), LinkedListNode::new(region));
            self.allocations.push(record);
            // SAFETY: `region.pointer` is a validated mmap result or cache entry, never null.
            NonNull::new_unchecked(region.pointer)
        }
    }

    pub fn dealloc_large(&mut self, pointer: *mut u8) {
        unsafe {
            let node = self.region_from_ptr(pointer);

            let region = node.as_ref().value;
            self.allocations.remove(node);
            self.metadata.dealloc(node);

            if !self.cache.park(region) {
                munmap(region.pointer, region.size_in_bytes);
            }
        }
    }

    /// Look up the mapped size of a live large allocation.
    pub fn allocation_size(&self, pointer: *mut u8) -> usize {
        unsafe { self.region_from_ptr(pointer).as_ref().value.size_in_bytes }
    }

    fn region_from_ptr(&self, pointer: *mut u8) -> NonNull<LinkedListNode<LargeRegion>> {
        unsafe {
            self.allocations
                .iter()
                .find(|node| node.as_ref().value.pointer == pointer)
                .unwrap_unchecked()
        }
    }
}

#[derive(Clone, Copy)]
pub struct LargeRegion {
    pub pointer: *mut u8,
    pub size_in_bytes: usize,
    pub zeroed: bool,
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
                zeroed: false,
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
    pub fn park(&mut self, mut region: LargeRegion) -> bool {
        if (self.entry_count as usize) >= CAPACITY {
            return false;
        }

        region.zeroed = false;
        self.entries[self.entry_count as usize] = region;
        self.entry_count += 1;
        true
    }
}
