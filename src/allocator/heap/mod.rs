use std::{
    ptr::{self, null_mut},
    sync::Mutex,
};

mod class_heap;
pub mod heap;
pub mod magazine;

use super::{
    global_class_regions, pseudorandom_bytes, ANONYMOUS_PRIVATE_MAP, DATA_PAGE_PROTECTION,
};
use crate::{allocator::heap::heap::Heap, libc::mem::mmap, page_size::round_up_to_page_size};

/// This thread's heap, installed eagerly at thread start so the fast path is a single `%fs`-relative load with no init check.
#[thread_local]
static mut HEAP_POINTER: *mut Heap = null_mut();

/// Free list of `Heap` structs orphaned by exited threads, recycled rather than re-`mmap`'d.
struct HeapFreeList(*mut Heap);
// SAFETY: the pointer is only dereferenced while holding the pool's `Mutex`.
unsafe impl Send for HeapFreeList {}
static FREE_HEAP_LIST: Mutex<HeapFreeList> = Mutex::new(HeapFreeList(null_mut()));

/// Creates this thread's heap and points `HEAP_POINTER` at it.
pub unsafe fn install_heap() {
    let storage = take_free_heap().unwrap_or_else(|| {
        let storage_bytes = round_up_to_page_size(size_of::<Heap>());
        let storage = mmap(
            null_mut(),
            storage_bytes,
            DATA_PAGE_PROTECTION,
            ANONYMOUS_PRIVATE_MAP,
            -1,
            0,
        ) as *mut Heap;
        assert!((storage as isize) > 0, "thread heap allocation failed");
        storage
    });

    ptr::write(storage, Heap::new(pseudorandom_bytes()));
    HEAP_POINTER = storage;
}

unsafe fn take_free_heap() -> Option<*mut Heap> {
    let mut pool = FREE_HEAP_LIST.lock().unwrap_unchecked();
    let head = pool.0;
    if head.is_null() {
        return None;
    }
    pool.0 = *(head as *const *mut Heap);
    Some(head)
}

/// Release the calling thread's heap and recycle its storage into the free list.
pub unsafe fn abandon_heap() {
    if HEAP_POINTER.is_null() {
        return;
    }

    let storage = HEAP_POINTER;
    (*storage).abandon_all(global_class_regions());
    HEAP_POINTER = null_mut();

    let mut pool = FREE_HEAP_LIST.lock().unwrap_unchecked();
    *(storage as *mut *mut Heap) = pool.0;
    pool.0 = storage;
}

#[inline(always)]
pub(super) unsafe fn get_heap() -> &'static mut Heap {
    debug_assert!(
        !HEAP_POINTER.is_null(),
        "allocation before install_thread_heap"
    );
    &mut *HEAP_POINTER
}
