use std::{
    ptr::{self, null_mut},
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::libc::mem::{mmap, mprotect, munmap, MapFlags, ProtectionFlags};

pub struct PageSpan {
    bytes: *mut [u8],
}

const READ_WRITE: ProtectionFlags = ProtectionFlags::ZERO
    .with_readable(true)
    .with_writable(true);

const NO_ACCESS: ProtectionFlags = ProtectionFlags::ZERO;

const ANONYMOUS_PRIVATE: MapFlags = MapFlags::ZERO.with_private(true).with_anonymous(true);

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
        let start = unsafe { mmap(null_mut(), size, READ_WRITE, ANONYMOUS_PRIVATE, -1, 0) };
        PageSpan {
            bytes: ptr::slice_from_raw_parts_mut(start, size),
        }
    }

    pub fn dealloc_pages(&self, page_span: PageSpan) {
        unsafe { munmap(page_span.bytes.as_mut_ptr(), page_span.bytes.len()) };
    }

    /// Allocate `page_count` usable pages with a guard page on each side.
    pub fn alloc_pages_guarded(&self, page_count: usize) -> PageSpan {
        let page_size = self.get_page_size();
        let total_pages = page_count + 2;
        let total_size = page_size * total_pages;

        // Map the entire region as read/write
        let region_start =
            unsafe { mmap(null_mut(), total_size, READ_WRITE, ANONYMOUS_PRIVATE, -1, 0) };

        // Mark the first and last pages as inaccessible
        let leading_guard = region_start;
        let trailing_guard = unsafe { region_start.add(page_size * (total_pages - 1)) };
        unsafe {
            mprotect(leading_guard, page_size, NO_ACCESS);
            mprotect(trailing_guard, page_size, NO_ACCESS);
        }

        // Return only the usable interior
        let usable_start = unsafe { region_start.add(page_size) };
        let usable_size = page_size * page_count;
        PageSpan {
            bytes: ptr::slice_from_raw_parts_mut(usable_start, usable_size),
        }
    }

    /// Free a guarded page span, including its surrounding guard pages.
    pub fn dealloc_pages_guarded(&self, page_span: PageSpan) {
        let page_size = self.get_page_size();
        let guard_start = unsafe { page_span.bytes.as_mut_ptr().sub(page_size) };
        let total_size = page_span.bytes.len() + (page_size * 2);
        unsafe { munmap(guard_start, total_size) };
    }
}
