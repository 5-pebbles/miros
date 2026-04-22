use std::ptr::null_mut;

use crate::{
    allocator::{
        metadata_allocator::MetadataAllocator,
        primary_allocator::{size_classes::SizeClass, span::Span},
        DATA_PAGE_PROTECTION,
    },
    libc::mem::mprotect,
    linked_list::{LinkedList, LinkedListNode},
    page_size,
};

/// Log2 of the per-class region size. 2^34 = 16 GB per class.
pub(super) const CLASS_REGION_SHIFT: u32 = 34;
pub(super) const CLASS_REGION_SIZE: usize = 1 << CLASS_REGION_SHIFT;

const RECENT_FREE_CAPACITY: usize = 16;

type EntryIndex = u8;
const _: () = assert!(RECENT_FREE_CAPACITY <= EntryIndex::MAX as usize);

struct RecentFreeCache {
    entries: [*mut u8; RECENT_FREE_CAPACITY],
    entry_count: EntryIndex,
}

impl RecentFreeCache {
    const fn new() -> Self {
        Self {
            entries: [null_mut(); RECENT_FREE_CAPACITY],
            entry_count: 0,
        }
    }

    fn random_pop(&mut self, random: u64) -> Option<*mut u8> {
        (self.entry_count != 0).then(|| {
            let index = (random as usize) % self.entry_count as usize;
            self.entry_count -= 1;
            self.entries.swap(index, self.entry_count as usize);
            self.entries[self.entry_count as usize]
        })
    }

    fn try_push(&mut self, entry: *mut u8) -> bool {
        if self.entry_count as usize >= RECENT_FREE_CAPACITY {
            return false;
        }

        self.entries[self.entry_count as usize] = entry;
        self.entry_count += 1;
        true
    }
}

/// Per-size-class state managing a contiguous virtual address region subdivided
/// into spans. Each class region occupies [`CLASS_REGION_SIZE`] bytes within the
/// super-region and lays spans back-to-back via a bump pointer.
///
/// Fully self-contained: owns its span metadata allocator and handles span
/// creation, reactivation, and release internally. The only external input is a
/// random `u64` for slot selection.
pub struct ClassRegion {
    size_class: SizeClass,
    base: *mut u8,
    /// Each span occupies `1 << span_stride_shift` bytes of virtual address
    /// space. The stride is the raw span size (2 * guard + data) rounded to
    /// the next power of two so pointer-to-span-number is a single shift.
    span_stride_shift: u32,
    next_span_offset: usize,
    /// Dense lookup table: `span_index[offset >> span_stride_shift]` yields the
    /// span metadata pointer. Stored as a fat pointer carrying the array length
    /// for debug bounds checks.
    span_index: *mut [*mut LinkedListNode<Span>],
    span_metadata: MetadataAllocator<LinkedListNode<Span>>,
    partial_spans: LinkedList<Span>,
    full_spans: LinkedList<Span>,
    empty_spans: LinkedList<Span>,
    recent_free: RecentFreeCache,
}

impl ClassRegion {
    pub unsafe fn new(
        size_class: SizeClass,
        base: *mut u8,
        span_stride_shift: u32,
        span_index: *mut [*mut LinkedListNode<Span>],
    ) -> Self {
        Self {
            size_class,
            base,
            span_stride_shift,
            next_span_offset: 0,
            span_index,
            span_metadata: MetadataAllocator::new(page_size::get_page_size()),
            partial_spans: LinkedList::new(),
            full_spans: LinkedList::new(),
            empty_spans: LinkedList::new(),
            recent_free: RecentFreeCache::new(),
        }
    }

    /// Allocate a slot from this class region. Returns null only if the region's
    /// 16 GB address space is exhausted — the caller should fall back to the
    /// large allocation path.
    pub unsafe fn alloc_slot(&mut self, random: u64) -> *mut u8 {
        if let Some(pointer) = self.recent_free.random_pop(random) {
            return pointer;
        }

        if self.partial_spans.is_empty() {
            if !self.empty_spans.is_empty() {
                self.reactivate_span();
            } else if !self.create_span() {
                return null_mut();
            }
        }

        let span_node = self.partial_spans.front();
        let span = &mut (*span_node).value;

        // SAFETY: we only reach here when partial_spans is non-empty, so the
        // front span is guaranteed to have at least one free slot.
        let slot_pointer = span.allocate_slot(random).unwrap_unchecked();

        if span.is_full() {
            (*span_node).list_remove();
            self.full_spans.list_push_front(span_node);
        }

        slot_pointer
    }

    /// Deallocate a slot by user pointer.
    pub unsafe fn dealloc_slot(&mut self, pointer: *mut u8) {
        if !self.recent_free.try_push(pointer) {
            self.dealloc_slot_slow(pointer);
        }
    }

    /* Private */

    /// Commit a fresh span within this region's virtual address space: mprotect
    /// the data pages RW, allocate span metadata, and push onto partial_spans.
    /// Returns false if the region is exhausted.
    unsafe fn create_span(&mut self) -> bool {
        let padded_stride = 1usize << self.span_stride_shift;
        if self.next_span_offset + padded_stride > CLASS_REGION_SIZE {
            return false;
        }

        // Guard region between spans: at least one page (for mprotect
        // granularity), but scaled to slot size for larger classes so a
        // single off-by-one doesn't silently land in the next span's data.
        let guard_size = self
            .size_class
            .slot_size_in_bytes()
            .max(page_size::get_page_size());
        let data_pointer = self.base.byte_add(self.next_span_offset + guard_size);
        let span_number = self.next_span_offset >> self.span_stride_shift;
        self.next_span_offset += padded_stride;

        mprotect(
            data_pointer,
            self.size_class.span_length_in_bytes(),
            DATA_PAGE_PROTECTION,
        );

        let span_node = self.span_metadata.alloc();
        core::ptr::write(
            span_node,
            LinkedListNode::new(Span::new(data_pointer, self.size_class)),
        );

        (&mut *self.span_index)[span_number] = span_node;
        self.partial_spans.list_push_front(span_node);
        true
    }

    #[cold]
    unsafe fn dealloc_slot_slow(&mut self, pointer: *mut u8) {
        let span_node = self.span_for_pointer(pointer);
        let span = &mut (*span_node).value;

        let was_full = span.is_full();
        span.dealloc_slot(pointer);

        if was_full {
            (*span_node).list_remove();
            if span.is_empty() {
                self.release_empty_span(span_node);
            } else {
                self.partial_spans.list_push_front(span_node);
            }
        } else if span.is_empty() {
            (*span_node).list_remove();
            self.release_empty_span(span_node);
        }
    }

    // TODO: when release_empty_span gains madvise(MADV_DONTNEED), this must
    // re-mprotect the data pages RW before reuse — keep these two in sync.
    unsafe fn reactivate_span(&mut self) {
        let span_node = self.empty_spans.front();
        (*span_node).list_remove();
        (*span_node).value.reinitialize();
        self.partial_spans.list_push_front(span_node);
    }

    unsafe fn release_empty_span(&mut self, span_node: *mut LinkedListNode<Span>) {
        // TODO: madvise(MADV_DONTNEED) on data pages to release physical memory
        self.empty_spans.list_push_front(span_node);
    }

    /// O(1) pointer-to-span lookup via the span index. Debug builds add bounds,
    /// null, and containment checks.
    unsafe fn span_for_pointer(&self, pointer: *mut u8) -> *mut LinkedListNode<Span> {
        let offset = pointer.addr() - self.base.addr();
        debug_assert!(
            offset < self.next_span_offset,
            "dealloc of pointer beyond allocated spans"
        );

        let span_number = offset >> self.span_stride_shift;
        let index = &*self.span_index;
        debug_assert!(
            span_number < index.len(),
            "span number exceeds index length"
        );

        let span_node = index[span_number];
        debug_assert!(!span_node.is_null(), "span_index entry is null");
        debug_assert!(
            (*span_node).value.contains_pointer(pointer),
            "pointer lands in guard page, not span data"
        );

        span_node
    }
}
