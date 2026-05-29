use std::ptr::null_mut;

/// An intrusive doubly-linked list.
///
/// Nodes are not owned by the list — they live in caller-controlled storage
/// (slab metadata, span headers, the TLS free map, ...) and are referenced by
/// raw pointer, so the list itself never allocates. Each node carries its own
/// `prev`/`next` links.
///
/// # Safety
///
/// The mutators are `unsafe` because the list cannot validate a raw pointer.
/// Callers must guarantee:
/// - every node pointer is non-null, aligned, and points to a live
///   `LinkedListNode<T>` that outlives its membership in the list.
/// - a node handed to `remove`/`insert_after` currently belongs to *this* list
///   (debug builds verify this by walking the chain);
/// - the list's structure is not mutated while an [`iter`](Self::iter) walk is in flight.
pub struct LinkedList<T> {
    head: *mut LinkedListNode<T>,
}

impl<T> LinkedList<T> {
    pub const fn new() -> Self {
        Self { head: null_mut() }
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// The front node, or null if the list is empty.
    pub fn front(&self) -> *mut LinkedListNode<T> {
        self.head
    }

    /// Link `node` at the front of the list.
    pub unsafe fn push_front(&mut self, node: *mut LinkedListNode<T>) {
        let old_head = self.head;
        (*node).prev = null_mut();
        (*node).next = old_head;
        if !old_head.is_null() {
            (*old_head).prev = node;
        }
        self.head = node;
    }

    /// Insert `node` immediately after `anchor`, which must already be in the list.
    pub unsafe fn insert_after(
        &mut self,
        anchor: *mut LinkedListNode<T>,
        node: *mut LinkedListNode<T>,
    ) {
        debug_assert!(
            self.contains(anchor),
            "insert_after: anchor not in this list"
        );
        let old_next = (*anchor).next;
        (*node).prev = anchor;
        (*node).next = old_next;
        (*anchor).next = node;
        if !old_next.is_null() {
            (*old_next).prev = node;
        }
    }

    /// Unlink `node`, which must currently belong to this list.
    pub unsafe fn remove(&mut self, node: *mut LinkedListNode<T>) {
        debug_assert!(self.contains(node), "remove: node not in this list");
        let prev = (*node).prev;
        let next = (*node).next;

        // The head write is the only one that touches the list itself, and it
        // goes through `&mut self` — never a stored raw pointer. The interior
        // links point node-to-node, into separate allocations the list doesn't
        // own, so writing them aliases no `&mut LinkedList`.
        if prev.is_null() {
            self.head = next;
        } else {
            (*prev).next = next;
        }
        if !next.is_null() {
            (*next).prev = prev;
        }

        (*node).prev = null_mut();
        (*node).next = null_mut();
    }

    /// Unlink and return the front node, or null if the list is empty.
    pub unsafe fn pop_front(&mut self) -> *mut LinkedListNode<T> {
        let node = self.head;
        if !node.is_null() {
            self.remove(node);
        }
        node
    }

    pub fn iter(&self) -> impl Iterator<Item = *mut LinkedListNode<T>> {
        let mut current = self.head;
        std::iter::from_fn(move || {
            let node = current;
            if node.is_null() {
                return None;
            }
            current = unsafe { (*node).next };
            Some(node)
        })
    }

    /// Membership walk backing the debug-only safety assertions. Compares
    /// pointers only — never dereferences `node` — so it stays safe even when
    /// `node` is stale or from another list.
    fn contains(&self, node: *mut LinkedListNode<T>) -> bool {
        !node.is_null() && self.iter().any(|candidate| candidate == node)
    }
}

#[repr(C)]
pub struct LinkedListNode<T> {
    pub value: T,
    prev: *mut LinkedListNode<T>,
    next: *mut LinkedListNode<T>,
}

impl<T> LinkedListNode<T> {
    pub const fn new(data: T) -> Self {
        Self {
            value: data,
            prev: null_mut(),
            next: null_mut(),
        }
    }
}
