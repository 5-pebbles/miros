use std::ptr;

use crate::{
    tls::TLS_RESERVE_SIZE,
    utils::{
        linked_list::{LinkedList, LinkedListNode},
        metadata_allocator::MetadataAllocator,
        round_up_to_boundary,
    },
};

struct FreeChunk {
    offset: usize,
    size: usize,
}

impl FreeChunk {
    fn top(&self) -> usize {
        self.offset + self.size
    }
}

/// Manages the TLS reserve region layout shared across all threads. Tracks
/// block allocations as offsets from the thread pointer — individual threads
/// apply these offsets to their own TP to derive addresses.
pub struct TlsLayoutAllocator {
    free: LinkedList<FreeChunk>,
    metadata: MetadataAllocator<LinkedListNode<FreeChunk>>,
}

impl TlsLayoutAllocator {
    pub fn new() -> Self {
        let mut metadata = MetadataAllocator::new();
        let mut free = LinkedList::new();

        let initial_chunk = metadata.alloc();
        unsafe {
            *initial_chunk = LinkedListNode::new(FreeChunk {
                offset: 0,
                size: TLS_RESERVE_SIZE,
            });
            free.push_front(initial_chunk);
        }

        Self { free, metadata }
    }

    pub unsafe fn allocate_block(&mut self, block_size: usize, alignment: usize) -> Option<isize> {
        let aligned_size = round_up_to_boundary(block_size, alignment);

        let node = self
            .free
            .iter()
            .filter(|&node| (*node).value.size >= aligned_size)
            .max_by_key(|&node| (*node).value.top())?;

        let chunk = &mut (*node).value;
        let aligned_start = chunk.top() - aligned_size;
        let remaining = aligned_start - chunk.offset;

        if remaining == 0 {
            self.free.remove(node);
            self.metadata.dealloc(node);
        } else {
            chunk.size = remaining;
        }

        Some(aligned_start as isize - TLS_RESERVE_SIZE as isize)
    }

    pub unsafe fn deallocate_block(&mut self, offset: isize, block_size: usize, alignment: usize) {
        let size = round_up_to_boundary(block_size, alignment);
        let region_offset = (offset + TLS_RESERVE_SIZE as isize) as usize;
        self.release(region_offset, size);
    }

    /// Returns a region to the free list, coalescing with adjacent free chunks.
    unsafe fn release(&mut self, offset: usize, size: usize) {
        let end = offset + size;

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
                (*prev).value.size = (*next).value.top() - (*prev).value.offset;
                self.free.remove(next);
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
                    self.free.insert_after(prev, new_node);
                }
            }
        }
    }
}
