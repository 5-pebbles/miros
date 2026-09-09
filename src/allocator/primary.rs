use std::{
    alloc::Layout,
    ptr::{self, null_mut, NonNull},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};

use super::{
    class_window::ClassWindow,
    heap::{get_heap, heap::HeapId},
    large_allocator::LargeAllocator,
    size_classes::{SizeClass, SIZE_CLASS_COUNT},
    span::Span,
    window_directory::WindowDirectory,
    ANONYMOUS_PRIVATE_MAP, GUARD_PAGE_PROTECTION,
};
use crate::{
    libc::mem::{mmap, munmap},
    utils::linked_list::{LinkedList, LinkedListNode},
};

/// Reserve PROT_NONE address space and trim it to an `alignment`-aligned mapping of `bytes`.
/// `None` when the kernel refuses the mapping.
unsafe fn reserve_aligned(bytes: usize, alignment: usize) -> Option<NonNull<u8>> {
    let raw = mmap(
        null_mut(),
        bytes + alignment,
        GUARD_PAGE_PROTECTION,
        ANONYMOUS_PRIVATE_MAP,
        -1,
        0,
    );
    if (raw as isize) <= 0 {
        return None;
    }

    let raw_address = raw.addr();
    let aligned_address = (raw_address + alignment - 1) & !(alignment - 1);
    let leading_slack = aligned_address - raw_address;
    let trailing_slack = alignment - leading_slack;

    if leading_slack > 0 {
        munmap(raw, leading_slack);
    }
    if trailing_slack > 0 {
        munmap(raw.add(leading_slack + bytes), trailing_slack);
    }

    // SAFETY: the aligned address sits inside a mapping the kernel just returned, so it is non-null.
    Some(NonNull::new_unchecked(raw.add(leading_slack)))
}

/// The shared half: every mutable field sits behind a lock or an atomic, so threads route through `&self`.
pub struct PrimaryAllocator {
    window_directory: WindowDirectory,
    /// Each class's newest window, where span carving starts. Replaced only by minting.
    current_windows: [AtomicUsize; SIZE_CLASS_COUNT],
    /// Orphaned spans pooled per class, mixing spans from every window of that class.
    abandoned: [Mutex<LinkedList<Span>>; SIZE_CLASS_COUNT],
    /// Serializes window minting only; span carving and routing stay lock-free.
    growth: Mutex<()>,
    large_allocator: Mutex<LargeAllocator>,
    pseudorandom_bytes: u128,
}

impl PrimaryAllocator {
    pub unsafe fn new(pseudorandom_bytes: [u8; 16]) -> Self {
        // Every class's first window comes from one reservation, so the common case never mints.
        let bootstrap_base =
            reserve_aligned(SIZE_CLASS_COUNT * ClassWindow::SIZE, ClassWindow::SIZE)
                .expect("bootstrap window reservation failed");

        let window_directory = WindowDirectory::new();
        let current_windows = std::array::from_fn(|class_index| {
            let size_class = SizeClass::from_raw(class_index as u8);
            let base = bootstrap_base.byte_add(class_index * ClassWindow::SIZE);
            let window = window_directory
                .mint(size_class, base)
                .expect("bootstrap window mint failed");
            AtomicUsize::new(window as *const ClassWindow as usize)
        });

        Self {
            window_directory,
            current_windows,
            abandoned: [const { Mutex::new(LinkedList::new()) }; SIZE_CLASS_COUNT],
            growth: Mutex::new(()),
            large_allocator: Mutex::new(LargeAllocator::new()),
            pseudorandom_bytes: u128::from_ne_bytes(pseudorandom_bytes),
        }
    }

    pub fn pseudorandom_bytes(&self) -> u128 {
        self.pseudorandom_bytes
    }

    #[inline(always)]
    pub unsafe fn alloc(&self, layout: Layout) -> Option<NonNull<u8>> {
        match SizeClass::from_layout(layout.size(), layout.align()) {
            Some(size_class) => self.alloc_small(size_class),
            None => self.alloc_large(layout),
        }
    }

    #[inline(always)]
    unsafe fn alloc_small(&self, size_class: SizeClass) -> Option<NonNull<u8>> {
        get_heap().alloc_small(self, size_class)
    }

    #[inline(always)]
    unsafe fn alloc_large(&self, layout: Layout) -> Option<NonNull<u8>> {
        self.large_allocator
            .lock()
            .unwrap_unchecked()
            .alloc_large(layout)
    }

    #[inline(always)]
    pub unsafe fn alloc_zeroed(&self, layout: Layout) -> Option<NonNull<u8>> {
        match SizeClass::from_layout(layout.size(), layout.align()) {
            Some(size_class) => {
                let pointer = self.alloc_small(size_class)?;
                ptr::write_bytes(pointer.as_ptr(), 0, size_class.slot_size_in_bytes());
                Some(pointer)
            }
            None => self.alloc_large_zeroed(layout),
        }
    }

    #[inline(always)]
    unsafe fn alloc_large_zeroed(&self, layout: Layout) -> Option<NonNull<u8>> {
        self.large_allocator
            .lock()
            .unwrap_unchecked()
            .alloc_large_zeroed(layout)
    }

    #[inline(always)]
    pub unsafe fn free(&self, pointer: *mut u8) {
        if pointer.is_null() {
            return;
        }
        match self.window_for_pointer(pointer) {
            Some(window) => self.dealloc_small(window, pointer),
            None => self.dealloc_large(pointer),
        }
    }

    pub unsafe fn dealloc(&self, pointer: *mut u8, _layout: Layout) {
        self.free(pointer)
    }

    #[inline(always)]
    unsafe fn dealloc_small(&self, window: &'static ClassWindow, pointer: *mut u8) {
        let span_node = window.span_for_pointer(pointer);
        let span = &span_node.as_ref().value;

        let heap = get_heap();
        if span.owner() == heap.id() {
            heap.dealloc_local(self, window.size_class(), pointer);
        } else {
            span.remote_dealloc_slot(pointer);
        }
    }

    #[inline(always)]
    unsafe fn dealloc_large(&self, pointer: *mut u8) {
        self.large_allocator
            .lock()
            .unwrap_unchecked()
            .dealloc_large(pointer)
    }

    pub unsafe fn realloc(&self, pointer: *mut u8, new_size: usize) -> Option<NonNull<u8>> {
        if pointer.is_null() {
            return self.alloc(Layout::from_size_align_unchecked(new_size, 1));
        }
        if new_size == 0 {
            self.free(pointer);
            return None;
        }

        let old_window = self.window_for_pointer(pointer);
        let old_class = old_window.map(|window| window.size_class());
        let new_class = SizeClass::from_layout(new_size, 1);

        if old_class.is_some() && old_class == new_class {
            // Same class: the existing block already satisfies the request.
            return Some(NonNull::new_unchecked(pointer));
        }

        let new_pointer = match new_class {
            Some(size_class) => self.alloc_small(size_class),
            None => self.alloc_large(Layout::from_size_align_unchecked(new_size, 1)),
        }?;

        let old_usable = match old_class {
            Some(class) => class.slot_size_in_bytes(),
            None => self
                .large_allocator
                .lock()
                .unwrap_unchecked()
                .allocation_size(pointer),
        };

        copy_realloc_payload(
            pointer,
            new_pointer.as_ptr(),
            old_usable.min(new_size),
            old_class,
        );

        match old_window {
            Some(window) => self.dealloc_small(window, pointer),
            None => self.dealloc_large(pointer),
        }
        Some(new_pointer)
    }

    /// The window containing `pointer`, or `None` when it is a large-path allocation.
    #[inline(always)]
    pub(super) fn window_for_pointer(&self, pointer: *mut u8) -> Option<&'static ClassWindow> {
        self.window_directory.lookup(pointer)
    }

    #[inline(always)]
    fn current_window(&self, size_class: SizeClass) -> &'static ClassWindow {
        let address = self.current_windows[size_class.index()].load(Ordering::Acquire);
        // SAFETY: set at init and only ever replaced with a newer live window, never null.
        unsafe { &*(address as *const ClassWindow) }
    }

    /// Carve a fresh span from the class's current window, minting a successor when it is exhausted.
    /// `None` only when the kernel refuses the reservation.
    #[cold]
    pub(super) unsafe fn create_span(
        &self,
        size_class: SizeClass,
        owner: HeapId,
    ) -> Option<NonNull<LinkedListNode<Span>>> {
        let mut window = self.current_window(size_class);
        loop {
            if let Some(span_node) = window.create_span(owner) {
                return Some(span_node);
            }
            window = self.mint_successor_window(size_class, window)?;
        }
    }

    /// Replace the exhausted window with a fresh one. The re-check under the lock keeps concurrent exhaustion in the same class from minting twice.
    /// `None` when the kernel refuses a mapping; a reservation whose mint failed stays mapped, which only matters at address-space exhaustion.
    #[cold]
    unsafe fn mint_successor_window(
        &self,
        size_class: SizeClass,
        exhausted: &ClassWindow,
    ) -> Option<&'static ClassWindow> {
        let _guard = self.growth.lock().unwrap_unchecked();

        let current = self.current_window(size_class);
        if !ptr::eq(current, exhausted) {
            // Another thread minted while we waited; the newer window has fresh space.
            return Some(current);
        }

        let window_base = reserve_aligned(ClassWindow::SIZE, ClassWindow::SIZE)?;
        let window = self.window_directory.mint(size_class, window_base)?;
        self.current_windows[size_class.index()]
            .store(window as *const ClassWindow as usize, Ordering::Release);
        Some(window)
    }

    /// Hand an exiting heap's per-class `list` to the abandoned pool, emptying it.
    pub(super) unsafe fn abandon_list(&self, size_class: SizeClass, list: &mut LinkedList<Span>) {
        self.abandoned[size_class.index()]
            .lock()
            .unwrap_unchecked()
            .prepend_adopt(list);
    }

    /// Claim one abandoned span for `new_owner`. Exactly one thread can claim any span.
    pub(super) unsafe fn adopt_span(
        &self,
        size_class: SizeClass,
        new_owner: HeapId,
    ) -> Option<NonNull<LinkedListNode<Span>>> {
        let span_node = self.abandoned[size_class.index()]
            .lock()
            .unwrap_unchecked()
            .pop()?;

        span_node.as_ref().value.set_owner(new_owner);
        Some(span_node)
    }
}

#[inline(always)]
unsafe fn copy_realloc_payload(
    source: *const u8,
    dest: *mut u8,
    copy_bytes: usize,
    old_class: Option<SizeClass>,
) {
    if let Some(class) = old_class {
        if copy_bytes == class.slot_size_in_bytes() {
            class.copy_slot(source, dest);
            return;
        }
    }
    ptr::copy_nonoverlapping(source, dest, copy_bytes);
}
