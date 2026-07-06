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

        // Node links live outside the list; only the `self.head` write aliases `&mut self`.
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

    /// Splice every node of `other` onto the front of this list, leaving `other` empty.
    pub unsafe fn prepend_adopt(&mut self, other: &mut LinkedList<T>) {
        let other_head = other.head;
        other.head = null_mut();
        if other_head.is_null() {
            return;
        }

        let mut tail = other_head;
        while !(*tail).next.is_null() {
            tail = (*tail).next;
        }

        (*tail).next = self.head;
        if !self.head.is_null() {
            (*self.head).prev = tail;
        }
        self.head = other_head;
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

    /// Membership walk backing the debug-only safety assertions.
    fn contains(&self, node: *mut LinkedListNode<T>) -> bool {
        // SAFETY:  Compares pointers only, never dereferences `node`, so it stays safe even when `node` is stale or from another list.
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

    pub fn next(&self) -> *mut LinkedListNode<T> {
        self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Owns the nodes and hands out one raw pointer per node, derived exactly once —
    /// re-borrowing a node would invalidate the list's stored pointer under Miri.
    struct Arena {
        _nodes: Vec<Box<LinkedListNode<i32>>>,
        pointers: Vec<*mut LinkedListNode<i32>>,
    }

    impl Arena {
        fn new(count: usize) -> Self {
            let mut nodes: Vec<Box<LinkedListNode<i32>>> = (0..count)
                .map(|value| Box::new(LinkedListNode::new(value as i32)))
                .collect();
            let pointers = nodes.iter_mut().map(|node| &mut **node as *mut _).collect();
            Self {
                _nodes: nodes,
                pointers,
            }
        }
    }

    unsafe fn values(list: &LinkedList<i32>) -> Vec<i32> {
        list.iter().map(|node| (*node).value).collect()
    }

    #[test]
    fn push_front_is_lifo() {
        let arena = Arena::new(3);
        let mut list = LinkedList::new();
        unsafe {
            for &node in &arena.pointers {
                list.push_front(node);
            }
            assert_eq!(values(&list), [2, 1, 0]);
            assert_eq!(list.front(), arena.pointers[2]);
        }
    }

    #[test]
    fn remove_front_middle_back() {
        let arena = Arena::new(4);
        let mut list = LinkedList::new();
        unsafe {
            for &node in &arena.pointers {
                list.push_front(node);
            }
            // list is [3, 2, 1, 0]
            list.remove(arena.pointers[3]); // front
            list.remove(arena.pointers[1]); // middle
            list.remove(arena.pointers[0]); // back
            assert_eq!(values(&list), [2]);
        }
    }

    #[test]
    fn pop_front_unlinks_then_empties() {
        let arena = Arena::new(2);
        let mut list = LinkedList::new();
        unsafe {
            list.push_front(arena.pointers[0]);
            list.push_front(arena.pointers[1]);
            assert_eq!(list.pop_front(), arena.pointers[1]);
            assert_eq!(list.pop_front(), arena.pointers[0]);
            assert!(list.pop_front().is_null());
            assert!(list.is_empty());
        }
    }

    #[test]
    fn insert_after_links_both_sides() {
        let arena = Arena::new(3);
        let mut list = LinkedList::new();
        unsafe {
            list.push_front(arena.pointers[0]);
            list.insert_after(arena.pointers[0], arena.pointers[1]); // [0, 1]
            list.insert_after(arena.pointers[0], arena.pointers[2]); // [0, 2, 1]
            assert_eq!(values(&list), [0, 2, 1]);
        }
    }

    #[test]
    fn prepend_splices_and_empties_other() {
        let first = Arena::new(2);
        let second = Arena::new(2);
        let mut target = LinkedList::new();
        let mut source = LinkedList::new();
        unsafe {
            target.push_front(first.pointers[0]);
            target.push_front(first.pointers[1]); // [1, 0]
            source.push_front(second.pointers[0]);
            source.push_front(second.pointers[1]); // [1, 0]

            target.prepend_adopt(&mut source);
            assert!(source.is_empty());
            // source's chain [1, 0] lands ahead of target's [1, 0]
            assert_eq!(values(&target).len(), 4);
        }
    }

    #[test]
    fn prepend_handles_empty_operands() {
        let arena = Arena::new(1);
        let mut target = LinkedList::new();
        let mut empty = LinkedList::new();
        unsafe {
            target.prepend_adopt(&mut empty); // empty into empty
            assert!(target.is_empty());

            target.push_front(arena.pointers[0]);
            target.prepend_adopt(&mut empty); // empty into non-empty
            assert_eq!(values(&target), [0]);
        }
    }

    /// The `reclaim_remote_frees` pattern: walk in place, reading `next` before each
    /// removal, relinking selected nodes into another list.
    #[test]
    fn in_place_walk_with_removal() {
        let arena = Arena::new(5);
        let mut source = LinkedList::new();
        let mut moved = LinkedList::new();
        unsafe {
            for &node in &arena.pointers {
                source.push_front(node);
            }
            // source is [4, 3, 2, 1, 0]
            let mut node = source.front();
            while !node.is_null() {
                let next = (*node).next();
                if (*node).value % 2 == 0 {
                    source.remove(node);
                    moved.push_front(node);
                }
                node = next;
            }
            assert_eq!(values(&source), [3, 1]);
            assert_eq!(values(&moved), [0, 2, 4]);
        }
    }
}
