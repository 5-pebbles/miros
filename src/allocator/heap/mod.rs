use std::{
    ptr::{self, null_mut, NonNull},
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

    storage.as_mut().abandon_all(global_class_regions());

    let mut pool = FREE_HEAP_LIST.lock().unwrap_unchecked();
    *(storage.as_ptr() as *mut Option<NonNull<Heap>>) = pool.0;
    pool.0 = Some(storage);
}

#[inline(always)]
pub(super) unsafe fn get_heap() -> &'static mut Heap {
    let heap_pointer = *(&raw const HEAP_POINTER);
    debug_assert!(
        heap_pointer.is_some(),
        "allocation before install_thread_heap"
    );
    heap_pointer.unwrap_unchecked().as_mut()
}
