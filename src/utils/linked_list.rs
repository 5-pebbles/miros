use std::ptr::NonNull;

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
    head: Option<NonNull<LinkedListNode<T>>>,
}

impl<T> LinkedList<T> {
    pub const fn new() -> Self {
        Self { head: None }
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    /// The front node, or None if the list is empty.
    pub fn front(&self) -> Option<NonNull<LinkedListNode<T>>> {
        self.head
    }

    /// Link `node` at the front of the list.
    pub unsafe fn push(&mut self, mut node: NonNull<LinkedListNode<T>>) {
        node.as_mut().prev = None;
        node.as_mut().next = self.head;
        if let Some(mut old_head) = self.head {
            old_head.as_mut().prev = Some(node);
        }
        self.head = Some(node);
    }

    /// Insert `node` immediately after `anchor`, which must already be in the list.
    pub unsafe fn insert_after(
        &mut self,
        mut anchor: NonNull<LinkedListNode<T>>,
        mut node: NonNull<LinkedListNode<T>>,
    ) {
        debug_assert!(
            self.contains(anchor),
            "insert_after: anchor not in this list"
        );
        let old_next = anchor.as_ref().next;
        node.as_mut().prev = Some(anchor);
        node.as_mut().next = old_next;
        anchor.as_mut().next = Some(node);
        if let Some(mut old_next) = old_next {
            old_next.as_mut().prev = Some(node);
        }
    }

    /// Unlink `node`, which must currently belong to this list.
    pub unsafe fn remove(&mut self, mut node: NonNull<LinkedListNode<T>>) {
        debug_assert!(self.contains(node), "remove: node not in this list");
        let prev = node.as_ref().prev;
        let next = node.as_ref().next;

        // Node links live outside the list; only the `self.head` write aliases `&mut self`.
        match prev {
            Some(mut prev) => prev.as_mut().next = next,
            None => self.head = next,
        }
        if let Some(mut next) = next {
            next.as_mut().prev = prev;
        }

        node.as_mut().prev = None;
        node.as_mut().next = None;
    }

    /// Unlink and return the front node, or None if the list is empty.
    pub unsafe fn pop(&mut self) -> Option<NonNull<LinkedListNode<T>>> {
        let node = self.head?;
        self.remove(node);
        Some(node)
    }

    /// Splice every node of `other` onto the front of this list, leaving `other` empty.
    pub unsafe fn prepend_adopt(&mut self, other: &mut LinkedList<T>) {
        let Some(other_head) = other.head.take() else {
            return;
        };

        let mut tail = other_head;
        while let Some(next) = tail.as_ref().next {
            tail = next;
        }

        tail.as_mut().next = self.head;
        if let Some(mut old_head) = self.head {
            old_head.as_mut().prev = Some(tail);
        }
        self.head = Some(other_head);
    }

    pub fn iter(&self) -> impl Iterator<Item = NonNull<LinkedListNode<T>>> {
        let mut current = self.head;
        std::iter::from_fn(move || {
            let Some(node) = current else {
                return None;
            };
            current = unsafe { node.as_ref().next };
            Some(node)
        })
    }

    /// Membership walk backing the debug-only safety assertions.
    fn contains(&self, node: NonNull<LinkedListNode<T>>) -> bool {
        // SAFETY:  Compares pointers only, never dereferences `node`, so it stays safe even when `node` is stale or from another list.
        self.iter().any(|candidate| candidate == node)
    }
}

#[repr(C)]
pub struct LinkedListNode<T> {
    pub value: T,
    prev: Option<NonNull<LinkedListNode<T>>>,
    next: Option<NonNull<LinkedListNode<T>>>,
}

impl<T> LinkedListNode<T> {
    pub const fn new(data: T) -> Self {
        Self {
            value: data,
            prev: None,
            next: None,
        }
    }

    pub fn next(&self) -> Option<NonNull<LinkedListNode<T>>> {
        self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Owns the nodes and hands out one `NonNull` per node, derived exactly once —
    /// re-borrowing a node would invalidate the list's stored pointer under Miri.
    struct Arena {
        _nodes: Vec<Box<LinkedListNode<i32>>>,
        pointers: Vec<NonNull<LinkedListNode<i32>>>,
    }

    impl Arena {
        fn new(count: usize) -> Self {
            let mut nodes: Vec<Box<LinkedListNode<i32>>> = (0..count)
                .map(|value| Box::new(LinkedListNode::new(value as i32)))
                .collect();
            let pointers = nodes
                .iter_mut()
                .map(|node| NonNull::from(&mut **node))
                .collect();
            Self {
                _nodes: nodes,
                pointers,
            }
        }
    }

    unsafe fn values(list: &LinkedList<i32>) -> Vec<i32> {
        list.iter().map(|node| node.as_ref().value).collect()
    }

    #[test]
    fn push_is_lifo() {
        let arena = Arena::new(3);
        let mut list = LinkedList::new();
        unsafe {
            for &node in &arena.pointers {
                list.push(node);
            }
            assert_eq!(values(&list), [2, 1, 0]);
            assert_eq!(list.front(), Some(arena.pointers[2]));
        }
    }

    #[test]
    fn remove_front_middle_back() {
        let arena = Arena::new(4);
        let mut list = LinkedList::new();
        unsafe {
            for &node in &arena.pointers {
                list.push(node);
            }
            // list is [3, 2, 1, 0]
            list.remove(arena.pointers[3]); // front
            list.remove(arena.pointers[1]); // middle
            list.remove(arena.pointers[0]); // back
            assert_eq!(values(&list), [2]);
        }
    }

    #[test]
    fn pop_unlinks_then_empties() {
        let arena = Arena::new(2);
        let mut list = LinkedList::new();
        unsafe {
            list.push(arena.pointers[0]);
            list.push(arena.pointers[1]);
            assert_eq!(list.pop(), Some(arena.pointers[1]));
            assert_eq!(list.pop(), Some(arena.pointers[0]));
            assert!(list.pop().is_none());
            assert!(list.is_empty());
        }
    }

    #[test]
    fn insert_after_links_both_sides() {
        let arena = Arena::new(3);
        let mut list = LinkedList::new();
        unsafe {
            list.push(arena.pointers[0]);
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
            target.push(first.pointers[0]);
            target.push(first.pointers[1]); // [1, 0]
            source.push(second.pointers[0]);
            source.push(second.pointers[1]); // [1, 0]

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

            target.push(arena.pointers[0]);
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
                source.push(node);
            }
            // source is [4, 3, 2, 1, 0]
            let mut cursor = source.front();
            while let Some(node) = cursor {
                let next = node.as_ref().next();
                if node.as_ref().value % 2 == 0 {
                    source.remove(node);
                    moved.push(node);
                }
                cursor = next;
            }
            assert_eq!(values(&source), [3, 1]);
            assert_eq!(values(&moved), [0, 2, 4]);
        }
    }
}
