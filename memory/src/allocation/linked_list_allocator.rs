//! Allocator that manages free memory with a linked list.

mod node;

use core::alloc::{AllocError, Allocator, Layout};
use core::cell::Cell;
use core::marker::PhantomData;
use core::ptr::NonNull;

use self::node::{Node, SplitNodeResult};
use super::{ExtendError, MemoryRegion, MemoryRegionAllocator};

/// A type of allocator that uses a linked list to manage free memory blocks.
#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub struct LinkedListAllocator {
    head: Cell<Option<NonNull<Node>>>,
    tail: Cell<Option<NonNull<Node>>>,
    coverage: MemoryRegion,
}

impl LinkedListAllocator {
    /// Creates an iterator over the free list.
    ///
    /// # Safety
    ///
    /// The iterator will need to create a reference to the nodes for iterating. For this reason,
    /// the linked list must be correctly set during the iteration and no nodes can be mutably
    /// aliased during the iteration steps, though they can be in between steps. The structure of
    /// the linked list also shouldn't change between iteration.
    unsafe fn iter(&self) -> Iter<'_> {
        Iter {
            current: self.head.get(),
            _phantom: PhantomData,
        }
    }

    /// Insets the node `next` after the node `prev` in the linked list, correctly linking their
    /// neighbors.
    ///
    /// # Safety
    ///
    /// The entire design depends on the linked list being correct. For this to make sense, `prev`
    /// shoud be part of the free list and `next` should not be part of the free list.
    ///
    /// # Panics
    ///
    /// if `prev.buffer().end() != next as *mut Node as *mut u8` (if the nodes aren't sorted in
    /// memory).
    unsafe fn insert_after(&self, prev: &mut Node, next: &mut Node) {
        if let Some(mut after) = prev.next() {
            Node::link(next, unsafe { after.as_mut() });
        }
        Node::link(prev, next);
        if prev as *mut Node == self.tail.get().unwrap().as_ptr() {
            self.tail.set(Some(next.into()));
        }
    }

    unsafe fn cover_node(&self, node: &mut Node) {
        unsafe {
            if let Some(mut prev) = node.prev() {
                prev.as_mut().set_next(node.next());
            } else {
                debug_assert_eq!(self.head.get().unwrap().as_ptr(), node as *mut Node);
                self.head.set(node.next());
                if self.head.get().is_none() {
                    self.tail.set(None);
                }
            }
        }
    }
}

struct Iter<'a> {
    current: Option<NonNull<Node>>,
    _phantom: PhantomData<&'a Node>,
}

impl Iterator for Iter<'_> {
    type Item = NonNull<Node>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current?;
        // SAFETY: No mutable references can exist during iteration (precondition).
        self.current = unsafe { current.as_ref() }.next();
        Some(current)
    }
}

unsafe impl Allocator for LinkedListAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let (mut node, split) = unsafe { self.iter() }
            .map(|node| (node, unsafe { node.as_ref() }.split_for_layout(layout)))
            .find(|(_, split)| {
                matches!(
                    split,
                    SplitNodeResult::Hijack | SplitNodeResult::Partition(_)
                )
            })
            .ok_or(AllocError {})?;

        // SAFETY: We don't have any more references into the list.
        let node = unsafe { node.as_mut() };

        match split {
            SplitNodeResult::Hijack => {}
            SplitNodeResult::Partition(at) => {
                let remainder = node.shrink_to(at).unwrap();
                // SAFETY: We've shrunk the previous node, so the region is completely managed by
                // the new Node.
                let (pre, next) = unsafe { Node::claim_region(remainder) }.unwrap();
                debug_assert!(
                    pre.is_empty(),
                    "Partition split should return the split with correct padding."
                );
                debug_assert!(
                    !next.buffer().is_empty(),
                    "Partition split could be hijacked."
                );

                // SAFETY: We maintin the linked list. Linking the original node with the new node
                // that came off from the partition.
                unsafe {
                    self.insert_after(node, next);
                }
            }
            SplitNodeResult::Misfit => {
                panic!("Shouldn't get here as we found a node with a different split.");
            }
        }

        unsafe {
            self.cover_node(node);
        }
        Ok(NonNull::slice_from_raw_parts(
            NonNull::new(node.buffer().start()).unwrap(),
            node.buffer().len(),
        ))
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        todo!();
    }
}

unsafe impl MemoryRegionAllocator for LinkedListAllocator {
    unsafe fn from_region(memory_region: MemoryRegion) -> Option<Self> {
        // SAFETY: We are passed ownership of the memory region.
        let (_pre, node) = unsafe { Node::claim_region(memory_region)? };
        Some(LinkedListAllocator {
            head: Cell::new(Some(node.into())),
            tail: Cell::new(Some(node.into())),
            coverage: memory_region,
        })
    }

    unsafe fn extend(&mut self, _size: usize) -> Result<(), ExtendError> {
        todo!();
    }

    fn coverage(&self) -> MemoryRegion {
        self.coverage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocation::test_utils::Arena;
    use crate::test_utils::init_logging;

    #[test]
    fn one_alloc() {
        init_logging();
        let arena = Arena::new(4096);
        let alloc = unsafe { LinkedListAllocator::from_region(arena.region()) }.unwrap();

        let a = Box::new_in(5, &alloc);
        assert_eq!(*a, 5);
    }

    #[test]
    fn multiple_alloc() {
        init_logging();
        let arena = Arena::new(4096);
        let alloc = unsafe { LinkedListAllocator::from_region(arena.region()) }.unwrap();

        let a = Box::new_in(5, &alloc);
        {
            let b = Box::new_in(6, &alloc);
            {
                let c = Box::new_in(7, &alloc);
                assert_eq!(*c, 7);
            }
            assert_eq!(*b, 6);
        }
        assert_eq!(*a, 5);
    }

    #[test]
    fn vec_growing_alloc() {
        init_logging();
        let arena = Arena::new(4096);
        let alloc = unsafe { LinkedListAllocator::from_region(arena.region()) }.unwrap();

        let mut v: Vec<usize, _> = Vec::new_in(&alloc);

        for i in 0..256 {
            v.push(i);
        }

        for (i, val) in v.iter().copied().enumerate() {
            assert_eq!(i, val);
        }
    }
}
