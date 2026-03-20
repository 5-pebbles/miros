use std::{
    ptr::{self, null_mut},
    sync::atomic::{AtomicUsize, Ordering},
};

use super::{ANONYMOUS_PRIVATE_MAP, DATA_PAGE_PROTECTION};
use crate::libc::mem::{mmap, munmap};

pub struct PageSpan {
    bytes: *mut [u8],
}

pub struct PageAllocator {
    page_size: AtomicUsize,
}

impl PageAllocator {
    pub const fn new() -> Self {
        Self {
            page_size: AtomicUsize::new(0),
        }
    }

    pub fn initialize(&mut self, page_size: usize) {
        debug_assert_eq!(self.page_size.load(Ordering::Acquire), 0);

        self.page_size.store(page_size, Ordering::Release);
    }

    pub fn get_page_size(&self) -> usize {
        self.page_size.load(Ordering::Relaxed)
    }

    pub fn alloc_pages(&self, page_count: usize) -> PageSpan {
        let size = self.get_page_size() * page_count;
        let start = unsafe {
            mmap(
                null_mut(),
                size,
                DATA_PAGE_PROTECTION,
                ANONYMOUS_PRIVATE_MAP,
                -1,
                0,
            )
        };
        PageSpan {
            bytes: ptr::slice_from_raw_parts_mut(start, size),
        }
    }

    pub fn dealloc_pages(&self, page_span: PageSpan) {
        unsafe { munmap(page_span.bytes.as_mut_ptr(), page_span.bytes.len()) };
    }
}
