use core::sync::atomic::{AtomicPtr, Ordering};

use linkme::distributed_slice;

use crate::objects::object_data_graph::ObjectDataGraph;

// Miros's manual GOT. -Bsymbolic pins direct accesses to exported data onto our own cells, so runtime-written exports (which an executable may COPY-relocate, taking ownership of the canonical copy) are accessed through a slot bound by normal search order at relocate time.
pub struct InterposableCell<T> {
    exported_name: &'static str,
    slot: AtomicPtr<T>,
}

impl<T> InterposableCell<T> {
    pub const fn new(exported_name: &'static str, own_cell: *mut T) -> Self {
        Self {
            exported_name,
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
    // The executable's COPY-created definition wins over our own export; absent one, this resolves back to our cell.
    fn bind(&self, graph: &ObjectDataGraph) {
        if let Ok(address) = graph.resolve_symbol_by_name(self.exported_name) {
            self.rebind(address.cast_mut().cast());
        }
    }
}

#[distributed_slice]
pub static INTERPOSABLE_CELLS: [&'static dyn Bindable];

pub fn bind_all(graph: &ObjectDataGraph) {
    INTERPOSABLE_CELLS.iter().for_each(|cell| cell.bind(graph));
}
