use std::{
    alloc::{AllocError, Allocator, Layout},
    ptr::{self, NonNull},
};

use crate::{
    libc::mem::{mmap, mremap, munmap, MapFlags, MremapFlags, ProtectionFlags},
    page_size::{get_page_size, round_up_to_page_size},
};

const MAP_FAILED: *mut u8 = usize::MAX as *mut u8;

fn non_null_or_map_failed(pointer: *mut u8, size: usize) -> Result<NonNull<[u8]>, AllocError> {
    if pointer == MAP_FAILED {
        return Err(AllocError);
    }
    unsafe {
        Ok(NonNull::new_unchecked(ptr::slice_from_raw_parts_mut(
            pointer, size,
        )))
    }
}

pub struct MreMapAllocator;

unsafe impl Allocator for MreMapAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(layout.align() <= get_page_size());

        let size = round_up_to_page_size(layout.size().max(1));
        let protection = ProtectionFlags::ZERO
            .with_readable(true)
            .with_writable(true);
        let flags = MapFlags::ZERO.with_private(true).with_anonymous(true);

        let pointer = unsafe { mmap(ptr::null_mut(), size, protection, flags, -1, 0) };
        non_null_or_map_failed(pointer, size)
    }

    unsafe fn deallocate(&self, pointer: NonNull<u8>, layout: Layout) {
        let size = round_up_to_page_size(layout.size().max(1));
        munmap(pointer.as_ptr(), size);
    }

    unsafe fn grow(
        &self,
        pointer: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        self.remap(pointer, old_layout, new_layout)
    }

    unsafe fn shrink(
        &self,
        pointer: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        self.remap(pointer, old_layout, new_layout)
    }
}

impl MreMapAllocator {
    unsafe fn remap(
        &self,
        pointer: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(new_layout.align() <= get_page_size());

        let old_size = round_up_to_page_size(old_layout.size().max(1));
        let new_size = round_up_to_page_size(new_layout.size().max(1));
        if new_size == old_size {
            return Ok(NonNull::new_unchecked(ptr::slice_from_raw_parts_mut(
                pointer.as_ptr(),
                new_size,
            )));
        }

        let flags = MremapFlags::ZERO.with_may_move(true);
        let result = mremap(pointer.as_ptr(), old_size, new_size, flags);
        non_null_or_map_failed(result, new_size)
    }
}
