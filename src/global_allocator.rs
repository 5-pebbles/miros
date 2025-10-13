use core::{
    alloc::{GlobalAlloc, Layout},
    cmp::max,
    ptr::null_mut,
    sync::atomic::{AtomicUsize, Ordering},
};
use std::ptr::copy_nonoverlapping;

use crate::{
    io_macros::syscall_debug_assert,
    start::auxiliary_vector::{AuxiliaryVectorIter, AT_PAGE_SIZE},
    static_pie::InitArrayFunction,
    syscall::mmap::{mmap, munmap, MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE},
};

#[link_section = ".init_array"]
pub(crate) static INIT_ALLOCATOR: InitArrayFunction = init_allocator;

extern "C" fn init_allocator(
    _arg_count: usize,
    _arg_pointer: *const *const u8,
    env_pointer: *const *const u8,
) {
    unsafe {
        let mut auxiliary_vector = AuxiliaryVectorIter::from_env_pointer(env_pointer);

        let page_size = auxiliary_vector
            .find(|item| item.a_type == AT_PAGE_SIZE)
            .unwrap()
            .a_un
            .a_val;

        ALLOCATOR.initialize(page_size);
    }
}

#[global_allocator]
pub(crate) static mut ALLOCATOR: Allocator = Allocator::new();

const MAX_SUPPORTED_ALIGN: usize = 4096;

pub(crate) struct Allocator {
    // I can't use OnceCell/OnceLock because they aren't sync
    page_size: AtomicUsize,

    thread_cache: ThreadCache,
}

impl Allocator {
    pub const fn new() -> Self {
        Allocator {
            page_size: AtomicUsize::new(0),

            thread_cache: ThreadCache::new(),
        }
    }

    pub fn initialize(&mut self, page_size: usize) {
        syscall_debug_assert!(self.page_size.load(Ordering::Relaxed) == 0);

        self.page_size.store(page_size, Ordering::Release);
    }

    fn align_layout_to_page_size(&self, layout: Layout) -> Layout {
        let page_size = self.page_size.load(Ordering::Acquire);

        let aligned_layout = layout.align_to(max(layout.align(), page_size));

        syscall_debug_assert!(aligned_layout.is_ok());

        aligned_layout.unwrap()
    }
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() > MAX_SUPPORTED_ALIGN {
            return null_mut();
        }

        let size = layout.pad_to_align().size();

        match size {
            _ => mmap(
                null_mut(),
                self.align_layout_to_page_size(layout).pad_to_align().size(),
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1, // file descriptor (-1 for anonymous mapping)
                0,  // offset
            ),
        }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        munmap(
            pointer,
            self.align_layout_to_page_size(layout).pad_to_align().size(),
        );
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ptr.is_null() {
            return self.alloc(Layout::from_size_align_unchecked(new_size, layout.align()));
        }

        if new_size == 0 {
            self.dealloc(ptr, layout);
            return null_mut();
        }

        if layout.align() > MAX_SUPPORTED_ALIGN {
            return null_mut();
        }

        let old_aligned_size = self.align_layout_to_page_size(layout).pad_to_align().size();
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let new_aligned_size = self
            .align_layout_to_page_size(new_layout)
            .pad_to_align()
            .size();

        if old_aligned_size == new_aligned_size {
            return ptr;
        }

        let new_ptr = self.alloc(new_layout);
        if new_ptr.is_null() {
            return null_mut();
        }

        copy_nonoverlapping(
            ptr,
            new_ptr,
            core::cmp::min(layout.pad_to_align().size(), new_size),
        );
        self.dealloc(ptr, layout);

        new_ptr
    }
}

struct ThreadCache {}

impl ThreadCache {
    pub const fn new() -> Self {
        Self {}
    }
}
