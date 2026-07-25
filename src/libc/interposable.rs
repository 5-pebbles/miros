use core::sync::atomic::{AtomicPtr, Ordering};

use linkme::distributed_slice;

use crate::objects::object_data_graph::ObjectDataGraph;

// Miros's manual GOT: runtime-written data exports that an executable may COPY-relocate, taking ownership of the canonical copy. -Bsymbolic pins direct accesses to our own cells, so access goes through a slot bound at load time. Each cell exports glibc's alias set (linked_aliases.def), and any of those names may be the one a program COPY-relocated.
pub struct InterposableCell<T> {
    exported_names: &'static [&'static str],
    slot: AtomicPtr<T>,
}

impl<T> InterposableCell<T> {
    pub const fn new(exported_names: &'static [&'static str], own_cell: *mut T) -> Self {
        Self {
            exported_names,
            slot: AtomicPtr::new(own_cell),
        }
    }

    pub(crate) fn rebind(&self, target: *mut T) {
        self.slot.store(target, Ordering::Relaxed);
    }

    pub fn as_ptr(&self) -> *mut T {
        self.slot.load(Ordering::Relaxed)
    }
}

pub trait Bindable: Sync {
    fn bind(&self, graph: &ObjectDataGraph);
}

impl<T> Bindable for InterposableCell<T> {
    fn bind(&self, graph: &ObjectDataGraph) {
        // A program that COPY-relocated any alias owns the canonical storage; route to it. No interposer is the common case: the slot stays on miros's own cell.
        let interposed = self
            .exported_names
            .iter()
            .find_map(|name| graph.resolve_symbol_outside_miros(name));

        if let Some(address) = interposed {
            self.rebind(address.cast_mut().cast());
        }
    }
}

#[distributed_slice]
pub static INTERPOSABLE_CELLS: [&'static dyn Bindable];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebind_redirects_stores() {
        let mut own = 0;
        let cell = InterposableCell::new(&["synthetic"], &raw mut own);
        unsafe { *cell.as_ptr() = 1 };

        let mut copied = 0;
        cell.rebind(&raw mut copied);
        unsafe { *cell.as_ptr() = 2 };

        assert_eq!(own, 1);
        assert_eq!(copied, 2);
    }
}
