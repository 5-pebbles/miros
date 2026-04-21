use std::ptr::{self, null_mut};

pub struct LinkedList<T> {
    head: *mut LinkedListNode<T>,
}

impl<T> LinkedList<T> {
    pub const fn new() -> Self {
        Self { head: null_mut() }
    }

    pub fn front(&self) -> *mut LinkedListNode<T> {
        self.head
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    pub unsafe fn list_push_front(&mut self, node: *mut LinkedListNode<T>) {
        let old_head = self.head;
        (*node).next = old_head;
        (*node).prevprev = ptr::from_mut(&mut self.head);
        if !old_head.is_null() {
            (*old_head).prevprev = ptr::from_mut(&mut (*node).next);
        }
        self.head = node;
    }
}

pub struct LinkedListNode<T> {
    next: *mut LinkedListNode<T>,
    prevprev: *mut *mut LinkedListNode<T>,
    pub value: T,
}

impl<T> LinkedListNode<T> {
    pub const fn new(data: T) -> Self {
        Self {
            next: null_mut(),
            prevprev: null_mut(),
            value: data,
        }
    }

    pub fn is_linked(&self) -> bool {
        !self.prevprev.is_null()
    }

    pub unsafe fn list_remove(&mut self) {
        debug_assert!(self.is_linked(), "removing node that is not in a list");
        *self.prevprev = self.next;
        if !self.next.is_null() {
            (*self.next).prevprev = self.prevprev;
        }
        self.next = null_mut();
        self.prevprev = null_mut();
    }
}
