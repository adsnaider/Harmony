//! Allocator that manages free memory with a linked list.

mod node;

use core::alloc::{AllocError, Allocator, Layout};
use core::cell::Cell;
use core::marker::PhantomData;
use core::ops::Range;
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

    unsafe fn insert_first(&self, node: &mut Node) {
        node.set_next(None);
        node.set_prev(None);
        if let Some(mut head) = self.head.get() {
            Node::link(node, unsafe { head.as_mut() });
        }

        self.head.set(Some(node.into()));
        if self.tail.get().is_none() {
            self.tail.set(Some(node.into()));
        }
    }

    unsafe fn unlink_node(&self, node: &mut Node) {
        unsafe {
            if let Some(mut prev) = node.prev() {
                prev.as_mut().set_next(node.next());
            }

            if let Some(mut next) = node.next() {
                next.as_mut().set_prev(node.prev());
            }
        }

        if self.head.get().unwrap() == node.into() {
            debug_assert!(node.prev().is_none());
            self.head.set(node.next());
        }
        if self.tail.get().unwrap() == node.into() {
            debug_assert!(node.next().is_none());
            self.tail.set(node.next());
        }

        node.set_next(None);
        node.set_prev(None);
    }

    unsafe fn coalesce_neighbors(
        left: NonNull<Node>,
        right: NonNull<Node>,
    ) -> Option<NonNull<Node>> {
        if unsafe { left.as_ref() }.buffer().end() as usize == right.as_ptr() as usize {
            let chunk = MemoryRegion::from_ptr_range(Range {
                start: left.as_ptr() as *mut u8,
                end: unsafe { right.as_ref() }.buffer().end(),
            });
            let left_neighbor = unsafe { left.as_ref().prev() };
            let right_neighbor = unsafe { right.as_ref().next() };

            let (pre, node) = unsafe { Node::claim_region(chunk).unwrap() };
            debug_assert!(pre.is_empty());
            if let Some(mut left_neighbor) = left_neighbor {
                Node::link(unsafe { left_neighbor.as_mut() }, node);
            }
            if let Some(mut right_neighbor) = right_neighbor {
                Node::link(node, unsafe { right_neighbor.as_mut() });
            }
            Some(node.into())
        } else {
            None
        }
    }

    unsafe fn coalesce(&self, mut node: NonNull<Node>) -> NonNull<Node> {
        unsafe {
            log::info!("Coallescing node {:?} ({:p})", node.as_ref(), node.as_ref());
            if let Some(prev) = node.as_ref().prev() {
                if let Some(new_node) = Self::coalesce_neighbors(prev, node) {
                    if self.tail.get().unwrap() == node {
                        self.tail.set(Some(new_node));
                    }
                    log::info!("Combined with prev");
                    node = new_node;
                }
            };
            if let Some(next) = node.as_ref().next() {
                if let Some(new_node) = Self::coalesce_neighbors(node, next) {
                    if self.head.get().unwrap() == node {
                        self.head.set(Some(new_node));
                    }
                    log::info!("Combined with next");
                    node = new_node;
                }
            };
            log::info!("Final node is {:?} ({:p})", node.as_ref(), node.as_ref());
            node
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
        log::info!("Requesting allocation for {:?}", layout);
        let (mut node, split) = unsafe { self.iter() }
            .map(|node| (node, unsafe { node.as_ref() }.split_for_layout(layout)))
            .find(|(_, split)| {
                matches!(
                    split,
                    SplitNodeResult::Hijack | SplitNodeResult::Partition(_)
                )
            })
            .ok_or(AllocError {})
            .inspect_err(|_| log::error!("Allocation error"))?;

        // SAFETY: We don't have any more references into the list.
        let node = unsafe { node.as_mut() };

        log::info!("Found suitable node: {:?} ({:p})", node, node);

        match split {
            SplitNodeResult::Hijack => {
                log::debug!("Hijacking node.")
            }
            SplitNodeResult::Partition(at) => {
                log::info!("Partition node at {}", at);
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
            self.unlink_node(node);
        }
        Ok(NonNull::slice_from_raw_parts(
            NonNull::new(node.buffer().start()).unwrap(),
            node.buffer().len(),
        ))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        log::info!(
            "Deallocation of {:p} with layout: {:?}",
            ptr.as_ptr(),
            layout
        );
        let node =
            unsafe { &mut *(ptr.as_ptr().wrapping_sub(core::mem::size_of::<Node>()) as *mut Node) };

        log::trace!("Got node {:?} ({:p})", node, node);

        let mut prev = None;
        for candidate in unsafe { self.iter() } {
            if ptr.as_ptr() < candidate.as_ptr() as *mut u8 {
                break;
            }
            prev = Some(candidate);
        }
        if let Some(mut prev) = prev {
            unsafe {
                log::info!(
                    "Linking node after {:?} ({:p})",
                    prev.as_ref(),
                    prev.as_ref()
                );
                self.insert_after(prev.as_mut(), node);
            }
        } else {
            unsafe {
                self.insert_first(node);
            }
        }

        unsafe {
            self.coalesce(node.into());
        }
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

    unsafe fn extend(&mut self, size: usize) -> Result<(), ExtendError> {
        if let Some(mut tail) = self.tail.get() {
            unsafe {
                if tail.as_ref().buffer().end() == self.coverage().end() {
                    tail.as_mut().grow(size)
                }
            }
        }
        let (_pad, last) = unsafe {
            Node::claim_region(MemoryRegion::from_addr_and_size(self.coverage.end(), size))
                .ok_or(ExtendError::Insufficient)?
        };
        if let Some(mut tail) = self.tail.get() {
            Node::link(unsafe { tail.as_mut() }, last);
        }
        self.tail.set(Some(last.into()));
        Ok(())
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

        let mut v: Vec<u32, _> = Vec::new_in(&alloc);

        for i in 0..256 {
            v.push(i);
        }

        for (i, val) in v.iter().copied().enumerate() {
            assert_eq!(i as u32, val);
        }
    }
}
