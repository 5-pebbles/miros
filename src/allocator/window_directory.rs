use std::{
    mem::size_of,
    ptr::{self, null_mut, NonNull},
};

use super::{
    class_window::ClassWindow, size_classes::SizeClass, ANONYMOUS_PRIVATE_MAP, DATA_PAGE_PROTECTION,
};
use crate::libc::mem::mmap;

/// Routes a pointer to its class window. Windows are 16 GB-aligned and 16 GB-sized, so the window number is the pointer's address shifted down, and one direct load resolves it.
/// Each slot is the `ClassWindow` itself, so the lookup's one load also fixes the window's fields.
///
/// A slot is minted under the allocator's growth lock. Nothing can read it during the mint: a reader only probes the slot of the slice its pointer lives in, the window's own reservation blocks every other mapping from that slice, and none of its slots have been handed out yet.
/// `publish` is therefore the only ordering the free path ever depends on.
pub(super) struct WindowDirectory {
    slots: NonNull<ClassWindow>,
}

impl WindowDirectory {
    /// One slot per 16 GB slice of the 47-bit user address space, indexed by `address >> 34`.
    /// All of miros's mappings stay below bit 47: unhinted mmaps on a 5-level-paging kernel still default to the 47-bit task size, and the directory bound-checks anyway.
    const SLOT_COUNT: usize = 1 << (47 - ClassWindow::SIZE_SHIFT);

    /// The slot array is kernel-zeroed, so every base reads 0 and every lookup misses.
    pub(super) unsafe fn new() -> Self {
        let directory_bytes = Self::SLOT_COUNT * size_of::<ClassWindow>();
        let slots = mmap(
            null_mut(),
            directory_bytes,
            DATA_PAGE_PROTECTION,
            ANONYMOUS_PRIVATE_MAP.with_noreserve(true),
            -1,
            0,
        );
        let slots = (slots as isize > 0)
            .then(|| slots as *mut ClassWindow)
            .expect("window directory mmap failed");
        Self {
            slots: NonNull::new_unchecked(slots),
        }
    }

    /// Mint a window into the directory slot its base address selects.
    /// `None` when the kernel refuses the metadata mapping or the base falls outside the directory.
    #[cold]
    pub(super) unsafe fn mint(
        &self,
        size_class: SizeClass,
        base: NonNull<u8>,
    ) -> Option<&'static ClassWindow> {
        let index = base.addr().get() >> ClassWindow::SIZE_SHIFT;
        if index >= Self::SLOT_COUNT {
            return None;
        }

        let slot = self.slots.as_ptr().add(index);
        let window = ClassWindow::new(size_class)?;
        ptr::write(slot, window);
        (*slot).publish(base);
        Some(&*slot)
    }

    /// The window containing `pointer`, or `None` when it lies outside every class window.
    #[inline(always)]
    pub(super) fn lookup(&self, pointer: *const u8) -> Option<&'static ClassWindow> {
        let index = pointer.addr() >> ClassWindow::SIZE_SHIFT;
        if index >= Self::SLOT_COUNT {
            return None;
        }

        // SAFETY: the index is bounds-checked above, and a minted slot is never reclaimed.
        let slot = unsafe { self.slots.as_ptr().add(index) };
        unsafe { (*slot).is_published().then(|| &*slot) }
    }
}
