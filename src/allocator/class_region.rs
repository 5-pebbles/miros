use std::{
    ptr::{self, null_mut},
    sync::Mutex,
};

use super::{size_classes::SizeClass, span::Span, ANONYMOUS_PRIVATE_MAP, DATA_PAGE_PROTECTION};
use crate::{
    allocator::heap::heap::HeapId,
    libc::mem::{mmap, mprotect},
    utils::linked_list::{LinkedList, LinkedListNode},
};

/// Log2 of the per-class region size. 2^34 = 16 GB per class.
pub(super) const CLASS_REGION_SHIFT: u32 = 34;
pub(super) const CLASS_REGION_SIZE: usize = 1 << CLASS_REGION_SHIFT;

struct SharedSpanPool {
    next_span_offset: usize,
    abandoned_spans: LinkedList<Span>,
}

// PERF: Padded to 256 bytes so `class_regions[i]` indexing compiles to a shift instead of a 3-cycle `imul`.
// Both `malloc` and `free` hit this on every call.
#[repr(C, align(256))]
pub struct ClassRegion {
    base: *mut u8,
    size_class: SizeClass,
    /// Each span occupies `1 << span_stride_shift` bytes,
    /// so spans tile the region and pointer-to-span number is a single shift.
    span_stride_shift: u32,
    metadata_base: *mut LinkedListNode<Span>,
    span_pool: Mutex<SharedSpanPool>,
}

impl ClassRegion {
    pub unsafe fn new(size_class: SizeClass, base: *mut u8) -> Self {
        let span_stride_shift = size_class.span_stride_shift();

        let max_spans = CLASS_REGION_SIZE >> span_stride_shift;
        // One inline node per span; NORESERVE keeps the range virtual until a span faults its page in.
        let metadata_byte_count = max_spans * core::mem::size_of::<LinkedListNode<Span>>();
        let metadata_base = mmap(
            null_mut(),
            metadata_byte_count,
            DATA_PAGE_PROTECTION,
            ANONYMOUS_PRIVATE_MAP.with_noreserve(true),
            -1,
            0,
        ) as *mut LinkedListNode<Span>;
        assert!((metadata_base as isize) > 0, "span metadata mmap failed");

        Self {
            base,
            size_class,
            span_stride_shift,
            metadata_base,
            span_pool: Mutex::new(SharedSpanPool {
                next_span_offset: 0,
                abandoned_spans: LinkedList::new(),
            }),
        }
    }

    /// O(1) pointer -> span, lock-free. Called by `free` on any thread. Carries no native synchronization of its own.
    pub unsafe fn span_for_pointer(&self, pointer: *mut u8) -> *mut LinkedListNode<Span> {
        let offset = pointer.addr() - self.base.addr();
        let span_number = offset >> self.span_stride_shift;
        debug_assert!(
            span_number < CLASS_REGION_SIZE >> self.span_stride_shift,
            "span number exceeds window capacity"
        );

        let span_node = self.metadata_base.add(span_number);
        debug_assert!(
            (*span_node).value.contains_pointer(pointer),
            "pointer outside its span's data range"
        );
        span_node
    }

    /// Carve a fresh span owned by `owner`. `None` when the 16 GB window is exhausted.
    #[cold]
    pub unsafe fn create_span(&self, owner: HeapId) -> Option<*mut LinkedListNode<Span>> {
        let padded_stride = 1usize << self.span_stride_shift;
        let next_span_offset = {
            let mut pool = self.span_pool.lock().unwrap_unchecked();
            let next_span_offset = pool.next_span_offset;

            // Test before advancing so an exhausted window never bumps the offset past the end.
            if next_span_offset + padded_stride > CLASS_REGION_SIZE {
                return None;
            }
            pool.next_span_offset += padded_stride;
            next_span_offset
        };

        let data_pointer = self.base.byte_add(next_span_offset);
        let span_number = next_span_offset >> self.span_stride_shift;

        mprotect(
            data_pointer,
            self.size_class.span_length_in_bytes(),
            DATA_PAGE_PROTECTION,
        );

        // Span N's node lives at a fixed offset in the metadata region;
        // the write faults its backing page in on first use.
        let span_node = self.metadata_base.add(span_number);
        ptr::write(
            span_node,
            LinkedListNode::new(Span::new(data_pointer, self.size_class, owner)),
        );
        Some(span_node)
    }

    /// Hand an exiting heap's per-class `list` to the abandoned pool, emptying it.
    pub unsafe fn abandon_list(&self, list: &mut LinkedList<Span>) {
        self.span_pool
            .lock()
            .unwrap_unchecked()
            .abandoned_spans
            .prepend_adopt(list);
    }

    /// Claim one abandoned span for `new_owner` — exactly one thread can claim any span.
    pub unsafe fn adopt_span(&self, new_owner: HeapId) -> Option<*mut LinkedListNode<Span>> {
        let span_node = {
            let mut pool = self.span_pool.lock().unwrap_unchecked();
            if pool.abandoned_spans.is_empty() {
                return None;
            }
            pool.abandoned_spans.pop_front()
        };

        (*span_node).value.set_owner(new_owner);
        Some(span_node)
    }
}
