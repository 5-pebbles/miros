use std::{
    ptr::{self, null_mut, NonNull},
    sync::{
        atomic::{Atomic, AtomicU64, Ordering},
        Mutex,
    },
};

mod class_heap;
mod magazine;

use self::{class_heap::ThreadClassHeap, magazine::Magazines};
use super::{
    non_crypto_rng::HeapRng,
    primary,
    primary::PrimaryAllocator,
    pseudorandom_bytes,
    size_classes::{SizeClass, SIZE_CLASS_COUNT},
    ANONYMOUS_PRIVATE_MAP, DATA_PAGE_PROTECTION,
};
use crate::{libc::mem::mmap, page_size::round_up_to_page_size};

/// This thread's heap, installed eagerly at thread start so the fast path is a single `%fs`-relative load with no init check.
#[thread_local]
static mut HEAP_POINTER: Option<NonNull<Heap>> = None;

/// Free list of `Heap` structs orphaned by exited threads, recycled rather than re-`mmap`'d.
struct HeapFreeList(Option<NonNull<Heap>>);
// SAFETY: the pointer is only dereferenced while holding the pool's `Mutex`.
unsafe impl Send for HeapFreeList {}
static FREE_HEAP_LIST: Mutex<HeapFreeList> = Mutex::new(HeapFreeList(None));

/// Creates this thread's heap and points `HEAP_POINTER` at it.
pub unsafe fn install_heap() {
    let storage = take_free_heap().unwrap_or_else(|| {
        let storage_bytes = round_up_to_page_size(size_of::<Heap>());
        let raw = mmap(
            null_mut(),
            storage_bytes,
            DATA_PAGE_PROTECTION,
            ANONYMOUS_PRIVATE_MAP,
            -1,
            0,
        ) as *mut Heap;
        NonNull::new(raw).expect("thread heap allocation failed")
    });

    ptr::write(storage.as_ptr(), Heap::new(pseudorandom_bytes()));
    HEAP_POINTER = Some(storage);
}

unsafe fn take_free_heap() -> Option<NonNull<Heap>> {
    let mut pool = FREE_HEAP_LIST.lock().unwrap_unchecked();
    let head = pool.0?;
    // Option<NonNull<T>> has the same bit representation as *mut T, so the intrusive
    // next-pointer stored in the abandoned heap's memory is directly reinterpretable.
    pool.0 = *(head.as_ptr() as *const Option<NonNull<Heap>>);
    Some(head)
}

/// Release the calling thread's heap and recycle its storage into the free list.
pub unsafe fn abandon_heap() {
    let Some(mut storage) = ptr::replace(&raw mut HEAP_POINTER, None) else {
        return;
    };

    storage.as_mut().abandon_all(primary());

    let mut pool = FREE_HEAP_LIST.lock().unwrap_unchecked();
    *(storage.as_ptr() as *mut Option<NonNull<Heap>>) = pool.0;
    pool.0 = Some(storage);
}

#[inline(always)]
pub(super) unsafe fn get_heap() -> &'static mut Heap {
    let heap_pointer = *(&raw const HEAP_POINTER);
    debug_assert!(heap_pointer.is_some(), "allocation before install_heap");
    heap_pointer.unwrap_unchecked().as_mut()
}

/// Identifies a heap. Monotonic and never reused, so a dead thread's spans never alias a live heap's id.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HeapId(u64);

/// A span's owner cell: any thread loads it to route a free, the owner stores it on adoption.
pub struct AtomicHeapId(Atomic<u64>);

impl AtomicHeapId {
    pub fn new(id: HeapId) -> Self {
        Self(Atomic::<u64>::new(id.0))
    }

    pub fn load(&self, order: Ordering) -> HeapId {
        HeapId(self.0.load(order))
    }

    pub fn store(&self, id: HeapId, order: Ordering) {
        self.0.store(id.0, order);
    }
}

// ⌊2^64 / φ⌋ forced odd: the multiply stays invertible (distinct ids never collide) and consecutive ids scatter maximally far apart.
const GOLDEN_RATIO_MULTIPLIER: u64 = 0x9E37_79B9_7F4A_7C15;

/// One per live thread, reached through a `#[thread_local]` pointer.
pub struct Heap {
    heap_id: HeapId,
    classes: [ThreadClassHeap; SIZE_CLASS_COUNT],
    magazines: Magazines,
    rng: HeapRng,
}

impl Heap {
    pub fn new(pseudorandom_bytes: u128) -> Self {
        static NEXT_HEAP_ID: AtomicU64 = AtomicU64::new(1);

        let raw_id = NEXT_HEAP_ID.fetch_add(1, Ordering::Relaxed);

        // Per-thread stream: mix the heap id into the process seed so threads don't share a draw sequence.
        // `| 1` keeps the xoroshiro state non-zero.
        let mixed = (raw_id as u128).wrapping_mul(GOLDEN_RATIO_MULTIPLIER as u128);
        let seed = (pseudorandom_bytes ^ (mixed << 64) ^ mixed) | 1;

        Self {
            heap_id: HeapId(raw_id),
            classes: [const { ThreadClassHeap::new() }; SIZE_CLASS_COUNT],
            magazines: Magazines::new(),
            rng: HeapRng::from_bytes(seed),
        }
    }

    pub fn id(&self) -> HeapId {
        self.heap_id
    }

    #[inline(always)]
    pub unsafe fn alloc_small(
        &mut self,
        primary: &PrimaryAllocator,
        size_class: SizeClass,
    ) -> Option<NonNull<u8>> {
        // Refill at the low-water mark so the draw always has a wide pool to randomize over.
        if self.magazines.needs_refill(size_class) {
            self.refill_class(size_class, primary);
        }
        self.magazines.class(size_class).draw_random(&mut self.rng)
    }

    #[cold]
    #[inline(never)]
    unsafe fn refill_class(&mut self, size_class: SizeClass, primary: &PrimaryAllocator) {
        let mut magazine = self.magazines.class(size_class);
        self.classes.get_mut(size_class.index()).unwrap().refill(
            &mut magazine,
            primary,
            size_class,
            self.heap_id,
            &mut self.rng,
        );
    }

    /// Stage the free in the magazine; a full magazine spills back to the span bitmap, in bulk.
    #[inline(always)]
    pub unsafe fn dealloc_local(
        &mut self,
        primary: &PrimaryAllocator,
        size_class: SizeClass,
        pointer: *mut u8,
    ) {
        let mut magazine = self.magazines.class(size_class);
        if !magazine.try_push(pointer) {
            self.classes
                .get_mut(size_class.index())
                .unwrap()
                .flush_to_span(&mut magazine, primary);
            // Flush drains to the low-water mark, so a slot is always free here.
            let pushed = magazine.try_push(pointer);
            debug_assert!(pushed, "magazine full immediately after flush");
        }
    }

    /// On thread exit, flush magazines back to their span bitmaps (still marked taken there), then abandon the spans.
    pub unsafe fn abandon_all(&mut self, primary: &PrimaryAllocator) {
        for class_index in 0..SIZE_CLASS_COUNT {
            let size_class = SizeClass::from_raw(class_index as u8);

            let mut magazine = self.magazines.class(size_class);
            while let Some(pointer) = magazine.pop() {
                // SAFETY: magazine slots were drawn from spans, so the window exists.
                let window = primary.window_for_pointer(pointer).unwrap_unchecked();
                let span_node = window.span_for_pointer(pointer);
                span_node.as_ref().value.dealloc_slot(pointer);
            }

            self.classes
                .get_mut(class_index)
                .unwrap()
                .abandon_all(primary, size_class);
        }
    }
}
