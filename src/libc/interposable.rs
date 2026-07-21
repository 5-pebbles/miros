use core::sync::atomic::{AtomicPtr, Ordering};

use linkme::distributed_slice;

use crate::{error::MirosError, objects::object_data_graph::ObjectDataGraph};

// Miros's manual GOT: runtime-written data exports that an executable may COPY-relocate, taking ownership of the canonical copy. -Bsymbolic pins direct accesses to our own cells, so access goes through a slot bound by normal search order at relocate time. Cells export under glibc's strong alias (ld dedups every reference onto it; rustc can't emit the weak twins).
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
    fn bind(&self, graph: &ObjectDataGraph) -> Result<(), MirosError>;
}

impl<T> Bindable for InterposableCell<T> {
    fn bind(&self, graph: &ObjectDataGraph) -> Result<(), MirosError> {
        // Miros exports every cell name, so a miss means an export vanished, not interposition.
        let address = graph.resolve_symbol_by_name(self.exported_name)?;
        self.rebind(address.cast_mut().cast());
        Ok(())
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
        let cell = InterposableCell::new("synthetic", &raw mut own);
        unsafe { *cell.as_ptr() = 1 };

        let mut copied = 0;
        cell.rebind(&raw mut copied);
        unsafe { *cell.as_ptr() = 2 };

        assert_eq!(own, 1);
        assert_eq!(copied, 2);
    }
}
