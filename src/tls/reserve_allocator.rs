use std::{mem::size_of, ptr};

use crate::{
    elf::thread_local_storage::DynamicThreadVectorItem,
    tls::TLS_RESERVE_SIZE,
    utils::{
        linked_list::{LinkedList, LinkedListNode},
        metadata_allocator::MetadataAllocator,
        round_up_to_boundary,
    },
};

struct FreeChunk {
    offset: isize,
    size: usize,
}

impl FreeChunk {
    fn top(&self) -> isize {
        self.offset + self.size as isize
    }
}

/// Manages the TLS reserve region shared across all threads. Tracks DTV and
/// block allocations as offsets from the thread pointer - individual threads
/// apply these offsets to their own TP to derive addresses.
pub struct TlsReserveAllocator {
    dtv_entry_count: usize,
    free: LinkedList<FreeChunk>,
    metadata: MetadataAllocator<LinkedListNode<FreeChunk>>,
}

impl TlsReserveAllocator {
    const RESERVE_BASE_OFFSET: isize = -(TLS_RESERVE_SIZE as isize);

    pub unsafe fn new() -> Self {
        let mut metadata = MetadataAllocator::new();
        let mut free = LinkedList::new();

        let initial_chunk = metadata.alloc();
        ptr::write(
            initial_chunk,
            LinkedListNode::new(FreeChunk {
                offset: Self::RESERVE_BASE_OFFSET,
                size: TLS_RESERVE_SIZE,
            }),
        );
        free.push_front(initial_chunk);

        Self {
            dtv_entry_count: 0,
            free,
            metadata,
        }
    }

    pub fn dtv(&mut self) -> DtvAllocator<'_> {
        DtvAllocator(self)
    }

    pub fn blocks(&mut self) -> BlockAllocator<'_> {
        BlockAllocator(self)
    }

    /// Returns a region to the free list, coalescing with adjacent free chunks.
    unsafe fn release(&mut self, offset: isize, size: usize) {
        let end = offset + size as isize;

        let mut prev: *mut LinkedListNode<FreeChunk> = ptr::null_mut();
        let mut next: *mut LinkedListNode<FreeChunk> = ptr::null_mut();

        for node in self.free.iter() {
            if (*node).value.offset >= end {
                next = node;
                break;
            }
            prev = node;
        }

        let merge_prev = !prev.is_null() && (*prev).value.top() == offset;
        let merge_next = !next.is_null() && (*next).value.offset == end;

        match (merge_prev, merge_next) {
            (true, true) => {
                (*prev).value.size = ((*next).value.top() - (*prev).value.offset) as usize;
                (*next).remove();
                self.metadata.dealloc(next);
            }
            (true, false) => {
                (*prev).value.size += size;
            }
            (false, true) => {
                (*next).value.offset = offset;
                (*next).value.size += size;
            }
            (false, false) => {
                let new_node = self.metadata.alloc();
                ptr::write(new_node, LinkedListNode::new(FreeChunk { offset, size }));
                if prev.is_null() {
                    self.free.push_front(new_node);
                } else {
                    (*prev).insert_after(new_node);
                }
            }
        }
    }
}

// DTV - bump allocator from base upward

pub struct DtvAllocator<'a>(&'a mut TlsReserveAllocator);

impl DtvAllocator<'_> {
    pub unsafe fn allocate(&mut self, new_entry_count: usize) -> bool {
        debug_assert!(new_entry_count > self.0.dtv_entry_count);

        let entry_size = size_of::<DynamicThreadVectorItem>();
        let additional = (new_entry_count - self.0.dtv_entry_count) * entry_size;
        let dtv_top = TlsReserveAllocator::RESERVE_BASE_OFFSET
            + (self.0.dtv_entry_count * entry_size) as isize;

        let Some(adjacent) = self
            .0
            .free
            .iter()
            .find(|&node| (*node).value.offset == dtv_top)
        else {
            return false;
        };

        if (*adjacent).value.size < additional {
            return false;
        }

        (*adjacent).value.offset += additional as isize;
        (*adjacent).value.size -= additional;

        if (*adjacent).value.size == 0 {
            (*adjacent).remove();
            self.0.metadata.dealloc(adjacent);
        }

        self.0.dtv_entry_count = new_entry_count;
        true
    }

    pub unsafe fn deallocate(&mut self, new_entry_count: usize) {
        debug_assert!(new_entry_count < self.0.dtv_entry_count);

        let entry_size = size_of::<DynamicThreadVectorItem>();
        let released_bytes = (self.0.dtv_entry_count - new_entry_count) * entry_size;
        let released_offset =
            TlsReserveAllocator::RESERVE_BASE_OFFSET + (new_entry_count * entry_size) as isize;

        self.0.release(released_offset, released_bytes);
        self.0.dtv_entry_count = new_entry_count;
    }

    pub fn offset(&self) -> isize {
        TlsReserveAllocator::RESERVE_BASE_OFFSET
    }

    pub fn entry_count(&self) -> usize {
        self.0.dtv_entry_count
    }
}

// TLS blocks - free list from top downward

pub struct BlockAllocator<'a>(&'a mut TlsReserveAllocator);

impl BlockAllocator<'_> {
    pub unsafe fn allocate(&mut self, block_size: usize, alignment: usize) -> Option<isize> {
        let aligned_size = round_up_to_boundary(block_size, alignment);

        let node = self
            .0
            .free
            .iter()
            .filter(|&node| (*node).value.size >= aligned_size)
            .max_by_key(|&node| (*node).value.top())?;

        let chunk = &mut (*node).value;
        let aligned_start = chunk.top() - aligned_size as isize;
        let remaining = (aligned_start - chunk.offset) as usize;

        if remaining == 0 {
            (*node).remove();
            self.0.metadata.dealloc(node);
        } else {
            chunk.size = remaining;
        }

        Some(aligned_start)
    }

    pub unsafe fn deallocate(&mut self, offset: isize, block_size: usize, alignment: usize) {
        let size = round_up_to_boundary(block_size, alignment);
        self.0.release(offset, size);
    }
}
