use std::ptr::NonNull;

use crate::{
    allocator::{
        heap::{magazine::Magazine, HeapId},
        non_crypto_rng::HeapRng,
        primary::PrimaryAllocator,
        size_classes::SizeClass,
        span::Span,
    },
    utils::linked_list::{LinkedList, LinkedListNode},
};

/// Per-thread, per-class span lists. Once owned, a span's bitmap and list membership are touched only by this thread.
pub(super) struct ThreadClassHeap {
    partial_spans: LinkedList<Span>,
    full_spans: LinkedList<Span>,
    /// The one hot reserve span, pages still resident.
    hot_reserve: Option<NonNull<LinkedListNode<Span>>>,
    // Holds only spans whose pages are discarded.
    empty_spans: LinkedList<Span>,
}

impl ThreadClassHeap {
    pub(super) const fn new() -> Self {
        Self {
            partial_spans: LinkedList::new(),
            full_spans: LinkedList::new(),
            hot_reserve: None,
            empty_spans: LinkedList::new(),
        }
    }

    /// Refill `magazine` to capacity from this class's spans.
    /// Returns early only when the kernel refuses fresh address space.
    #[cold]
    pub(super) unsafe fn refill(
        &mut self,
        magazine: &mut Magazine,
        primary: &PrimaryAllocator,
        size_class: SizeClass,
        owner: HeapId,
        random: &mut HeapRng,
    ) {
        while magazine.remaining_capacity() > 0 {
            if self.partial_spans.is_empty() && !self.replenish_partial(primary, size_class, owner)
            {
                return;
            }
            let span_node = self.partial_spans.front().unwrap_unchecked();
            let span = &span_node.as_ref().value;

            // Fold in cross-thread frees first so their slots re-enter this batch instead of forcing fresh address space.
            // An idle thread strands remote frees until its next refill or exit. Only the owner may reuse them.
            if span.has_remote_frees() {
                span.reclaim_remote_frees();
            }

            // A partial span always has a free slot, so the claim succeeds. Fresh entropy per claim scatters the batch across the span.
            let claimed = span
                .claim_up_to(magazine.remaining_capacity() as u32, random.next_u64())
                .unwrap_unchecked();
            magazine.refill(claimed);

            if span.is_full() {
                self.partial_spans.remove(span_node);
                self.full_spans.push(span_node);
            }
        }
    }

    /// Ensure `partial_spans` is non-empty. `false` only when the kernel refuses fresh address space.
    unsafe fn replenish_partial(
        &mut self,
        primary: &PrimaryAllocator,
        size_class: SizeClass,
        owner: HeapId,
    ) -> bool {
        loop {
            if !self.partial_spans.is_empty() {
                return true;
            }
            // Reclaim remote frees before spending fresh address space.
            if self.reclaim_remote_frees() {
                continue;
            }
            if self.reactivate_span() {
                continue;
            }
            if self.adopt_span(primary, size_class, owner) {
                continue;
            }
            match primary.create_span(size_class, owner) {
                Some(span_node) => self.partial_spans.push(span_node),
                None => return false,
            }
        }
    }

    /// Drain the magazine's overflow down to its low-water mark, returning slots to spans in bulk.
    pub(super) unsafe fn flush_to_span(
        &mut self,
        magazine: &mut Magazine,
        primary: &PrimaryAllocator,
    ) {
        while let Some(pointer) = magazine.pop_above_low_water() {
            // SAFETY: magazine slots were drawn from spans, so the window exists.
            let window = primary.window_for_pointer(pointer).unwrap_unchecked();
            let span_node = window.span_for_pointer(pointer);
            self.dealloc_to_span(span_node, pointer);
        }
    }

    /// Return one slot to its span bitmap and fix up list membership.
    unsafe fn dealloc_to_span(
        &mut self,
        span_node: NonNull<LinkedListNode<Span>>,
        pointer: *mut u8,
    ) {
        let span = &span_node.as_ref().value;

        let was_full = span.is_full();
        span.dealloc_slot(pointer);

        if was_full {
            self.full_spans.remove(span_node);
            self.place_span(span_node);
        } else if span.is_empty() {
            self.partial_spans.remove(span_node);
            self.place_span(span_node);
        }
    }

    /// File a span with freshly-updated occupancy into the matching list. The node must be unlinked first.
    unsafe fn place_span(&mut self, span_node: NonNull<LinkedListNode<Span>>) {
        let span = &span_node.as_ref().value;
        if span.is_empty() {
            self.release_empty_span(span_node);
        } else if span.is_full() {
            self.full_spans.push(span_node);
        } else {
            self.partial_spans.push(span_node);
        }
    }

    /// Drain remote frees out of this thread's full spans, relisting any that regained slots.
    /// `next` is read before each `remove`, so the in-place walk stays valid.
    unsafe fn reclaim_remote_frees(&mut self) -> bool {
        let mut made_progress = false;
        let mut node = self.full_spans.front();

        while let Some(current) = node {
            let next = current.as_ref().next();
            let span = &current.as_ref().value;

            if span.has_remote_frees() {
                // Draining a full span frees >= 1 slot, so it is no longer full: relist it unconditionally.
                span.reclaim_remote_frees();
                self.full_spans.remove(current);
                self.place_span(current);
                made_progress = true;
            }

            node = next;
        }
        made_progress
    }

    /// Claim one abandoned span, draining any frees it accumulated while orphaned.
    unsafe fn adopt_span(
        &mut self,
        primary: &PrimaryAllocator,
        size_class: SizeClass,
        owner: HeapId,
    ) -> bool {
        let span_node = match primary.adopt_span(size_class, owner) {
            Some(span_node) => span_node,
            None => return false,
        };

        let span = &span_node.as_ref().value;
        span.reclaim_remote_frees();
        self.place_span(span_node);
        true
    }

    pub(super) unsafe fn abandon_all(&mut self, primary: &PrimaryAllocator, size_class: SizeClass) {
        primary.abandon_list(size_class, &mut self.partial_spans);
        primary.abandon_list(size_class, &mut self.full_spans);
        primary.abandon_list(size_class, &mut self.empty_spans);
    }

    /// The reserve is the hot path; everything in `empty_spans` re-faults on reactivation.
    unsafe fn reactivate_span(&mut self) -> bool {
        let Some(span_node) = self.hot_reserve.take().or_else(|| self.empty_spans.pop()) else {
            return false;
        };
        span_node.as_ref().value.reinitialize();
        self.partial_spans.push(span_node);
        true
    }

    /// The first empty span becomes the hot reserve; the previous reserve is demoted and its pages discarded.
    unsafe fn release_empty_span(&mut self, span_node: NonNull<LinkedListNode<Span>>) {
        let Some(demoted) = self.hot_reserve.replace(span_node) else {
            return;
        };

        demoted.as_ref().value.discard_data_pages();
        self.empty_spans.push(demoted);
    }
}
