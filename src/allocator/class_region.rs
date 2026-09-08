use std::{
    mem::size_of,
    ptr::{self, null_mut, NonNull},
    sync::atomic::{AtomicUsize, Ordering},
};

use super::{size_classes::SizeClass, span::Span, ANONYMOUS_PRIVATE_MAP, DATA_PAGE_PROTECTION};
use crate::{
    allocator::heap::heap::HeapId,
    libc::mem::{mmap, mprotect},
    utils::linked_list::LinkedListNode,
};

/// Log2 of the per-window size. 2^34 = 16 GB, the granularity at which a class grows.
pub(super) const CLASS_REGION_SHIFT: u32 = 34;
pub(super) const CLASS_REGION_SIZE: usize = 1 << CLASS_REGION_SHIFT;

/// One class's slice of address space: a 16 GB window plus its span metadata.
/// Lives inside a `WindowDirectory` slot; `base` doubles as the publication flag.
pub struct ClassRegion {
    /// Zero until `publish` runs, which is why an empty directory slot reads as a miss.
    base: AtomicUsize,
    size_class: SizeClass,
    /// Each span occupies `1 << span_stride_shift` bytes, so spans tile the window and pointer-to-span number is a single shift.
    span_stride_shift: u32,
    metadata_base: NonNull<LinkedListNode<Span>>,
    /// Offset of the next uncarved span. Monotonic; never advances past the window's end.
    span_cursor: AtomicUsize,
}

impl ClassRegion {
    /// `None` when the kernel refuses the metadata mapping. `base` stays unpublished.
    pub(super) unsafe fn new(size_class: SizeClass) -> Option<Self> {
        let span_stride_shift = size_class.span_stride_shift();

        let max_spans = CLASS_REGION_SIZE >> span_stride_shift;
        // One inline node per span; NORESERVE keeps the range virtual until a span faults its page in.
        let metadata_byte_count = max_spans * size_of::<LinkedListNode<Span>>();
        let metadata = mmap(
            null_mut(),
            metadata_byte_count,
            DATA_PAGE_PROTECTION,
            ANONYMOUS_PRIVATE_MAP.with_noreserve(true),
            -1,
            0,
        );
        if (metadata as isize) <= 0 {
            return None;
        }

        Some(Self {
            base: AtomicUsize::new(0),
            size_class,
            span_stride_shift,
            metadata_base: NonNull::new_unchecked(metadata as *mut LinkedListNode<Span>),
            span_cursor: AtomicUsize::new(0),
        })
    }

    /// The mint's last write: the `Release` store pairs with `lookup`'s `Acquire` load, so a reader that sees a nonzero base sees the whole region.
    pub(super) fn publish(&self, base: NonNull<u8>) {
        self.base.store(base.addr().get(), Ordering::Release);
    }

    /// A directory slot answering false reads as a miss.
    pub(super) fn is_published(&self) -> bool {
        self.base.load(Ordering::Acquire) != 0
    }

    pub(super) fn size_class(&self) -> SizeClass {
        self.size_class
    }

    /// O(1) pointer -> span, lock-free. Called by `free` on any thread. Carries no native synchronization of its own.
    pub unsafe fn span_for_pointer(&self, pointer: *mut u8) -> NonNull<LinkedListNode<Span>> {
        let offset = pointer.addr() - self.base.load(Ordering::Relaxed);
        let span_number = offset >> self.span_stride_shift;
        debug_assert!(
            span_number < CLASS_REGION_SIZE >> self.span_stride_shift,
            "span number exceeds window capacity"
        );

        let span_node = self.metadata_base.add(span_number);
        debug_assert!(
            span_node.as_ref().value.contains_pointer(pointer),
            "pointer outside its span's data range"
        );
        span_node
    }

    /// Carve a fresh span owned by `owner`. `None` when this window is exhausted; the caller mints a successor window rather than failing the allocation.
    #[cold]
    pub unsafe fn create_span(&self, owner: HeapId) -> Option<NonNull<LinkedListNode<Span>>> {
        let padded_stride = 1usize << self.span_stride_shift;

        // NOTE: Relaxed ordering suffices: a fresh span's metadata reaches other threads through the pointers it hands out, never through the cursor.
        // Advance only while the next span still fits, so the cursor never leaves the window.
        let next_span_offset = self
            .span_cursor
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |cursor| {
                let next = cursor + padded_stride;
                (next <= CLASS_REGION_SIZE).then_some(next)
            })
            .ok()?;

        let data_pointer = NonNull::new_unchecked(
            (self.base.load(Ordering::Relaxed) + next_span_offset) as *mut u8,
        );
        let span_number = next_span_offset >> self.span_stride_shift;

        mprotect(
            data_pointer.as_ptr(),
            self.size_class.span_length_in_bytes(),
            DATA_PAGE_PROTECTION,
        );

        // Span N's node lives at a fixed offset in the metadata region;
        // the write faults its backing page in on first use.
        let span_node = self.metadata_base.add(span_number);
        ptr::write(
            span_node.as_ptr(),
            LinkedListNode::new(Span::new(data_pointer, self.size_class, owner)),
        );
        Some(span_node)
    }
}
