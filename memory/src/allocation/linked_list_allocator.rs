//! Allocator that manages free memory with a linked list.

mod node;

use core::alloc::{AllocError, Allocator, Layout};
use core::marker::PhantomData;
use core::ops::Range;
use core::ptr::NonNull;

use self::node::{Node, SplitNodeResult};
use super::{ExtendError, MemoryRegion, MemoryRegionAllocator};

/// A type of allocator that uses a linked list to manage free memory blocks.
#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub struct LinkedListAllocator {
    /// Head of the linked list allocator (sentinel).
    head: NonNull<Node>,
    /// Tail of the linked list allocator (sentinel).
    tail: NonNull<Node>,
    /// The coverage of the allocator.
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
            current: unsafe { self.head.as_ref().next().unwrap() },
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
    /// Additionally, no references should exist anywhere accross the list (other than `prev` and
    /// `next`) during the function call.
    ///
    /// # Panics
    ///
    /// If the node inserted causes the list to get out of order.
    unsafe fn insert_after(prev: &mut Node, next: &mut Node) {
        if !prev.is_sentinel() && prev.buffer().start() > next.buffer().start() {
            panic!(
                "Inserting {:?} ({:p}) after {:?} ({:p}) would cause the list to get out of order.",
                next, next, prev, prev
            );
        }
        let after = unsafe { prev.next().unwrap().as_mut() };
        if !after.is_sentinel() && next.buffer().start() > after.buffer().start() {
            panic!(
                "Inserting {:?} ({:p}) after {:?} ({:p}) would cause the list to get out of order because the following node {:?} ({:p}) should be behind.",
                next, next, prev, prev, after, after);
        }
        Node::link(next, after);
        Node::link(prev, next);
    }

    /// Inserts a node to the end of the list (right before the `tail` sentinel).
    ///
    /// # Safety
    ///
    /// No references can exist to any nodes in the list during the functio call (other than
    /// `node`).
    ///
    /// # Panics
    ///
    /// If the node inserted causes the list to get out of order.
    unsafe fn insert_last(&self, node: &mut Node) {
        unsafe {
            Self::insert_after(self.tail.as_ref().prev().unwrap().as_mut(), node);
        }
    }

    /// Inserts a node to the beginning of the list (right after the `head` sentinel).
    ///
    /// # Safety
    ///
    ///  No references can exist to any nodes in the list during the functio call (other than
    /// `node`).
    ///
    /// # Panics
    ///
    /// If the node inserted causes the list to get out of order.
    unsafe fn insert_first(&self, node: &mut Node) {
        unsafe {
            Self::insert_after(&mut *self.head.as_ptr(), node);
        }
    }

    /// Unlinks a node from the list.
    ///
    /// This method causes `node`'s neighbors to skip through `node`. Additionally, the method will
    /// clear out the links from `node`.
    ///
    /// # Safety
    ///
    /// No references must exist to any nodes other than `node` in the list.
    ///
    /// # Panics
    ///
    /// If the node is a `sentinel` node.
    unsafe fn unlink_node(&self, node: &mut Node) {
        Node::link(unsafe { node.prev().unwrap().as_mut() }, unsafe {
            node.next().unwrap().as_mut()
        });

        node.set_next(None);
        node.set_prev(None);
    }

    /// Coalesces 2 nodes if they are contiguous in memory.
    ///
    /// # Safety
    ///
    /// * After the function returns, if the result is Some, then the returned node can be
    ///   derreferenced but neither one of the parameters should be dereferenced.
    /// * Additionally, no references can exist to either of the nodes or the neighbors when
    ///   calling this function.
    /// * Lastly, this function expects both `left` and `right` to be true neighbors in the list.
    ///
    /// # Panics
    ///
    /// If either `left` or `right` are the sentinel nodes.
    unsafe fn coalesce_neighbors(
        left: NonNull<Node>,
        right: NonNull<Node>,
    ) -> Option<NonNull<Node>> {
        if unsafe { left.as_ref() }.buffer().end() as usize == right.as_ptr() as usize {
            let chunk = MemoryRegion::from_ptr_range(Range {
                start: left.as_ptr() as *mut u8,
                end: unsafe { right.as_ref() }.buffer().end(),
            });
            let left_neighbor = unsafe { left.as_ref().prev().unwrap().as_mut() };
            let right_neighbor = unsafe { right.as_ref().next().unwrap().as_mut() };

            let (pre, node) = unsafe { Node::claim_region(chunk).unwrap() };
            debug_assert!(pre.is_empty());
            Node::link(left_neighbor, node);
            Node::link(node, right_neighbor);
            Some(node.into())
        } else {
            None
        }
    }

    /// Coalesces a node with it's left and rigth neighbors.
    ///
    /// # Safety
    ///
    /// * No references can exist into the list at the call of this function.
    /// * `node` must be a valid node in the list.
    ///
    /// # Panics
    ///
    /// If `node` is either sentinel node.
    unsafe fn coalesce(&self, mut node: NonNull<Node>) -> NonNull<Node> {
        unsafe {
            log::info!("Coallescing node {:?} ({:p})", node.as_ref(), node.as_ref());
            let prev = node.as_ref().prev().unwrap();
            if !prev.as_ref().is_sentinel() {
                if let Some(n) = Self::coalesce_neighbors(prev, node) {
                    node = n;
                }
            }
            let next = node.as_ref().next().unwrap();
            if !next.as_ref().is_sentinel() {
                if let Some(n) = Self::coalesce_neighbors(node, next) {
                    node = n;
                }
            }

            log::info!("Final node is {:?} ({:p})", node.as_ref(), node.as_ref());
            node
        }
    }
}

/// Iterator over the linked list allocator.
///
/// This iterator will start at head->next and be done when it hits the tail of the list. In
/// particular, the iterator will never return a sentinel node.
struct Iter<'a> {
    /// The next node to return.
    current: NonNull<Node>,
    /// phantom for lifetimes.
    _phantom: PhantomData<&'a Node>,
}

impl Iterator for Iter<'_> {
    type Item = NonNull<Node>;

    fn next(&mut self) -> Option<Self::Item> {
        if unsafe { self.current.as_ref().is_sentinel() } {
            return None;
        }
        let current = Some(self.current);
        // SAFETY: No mutable references can exist during iteration (precondition).
        self.current = unsafe { self.current.as_ref() }.next().unwrap();
        current
    }
}

// TODO(adsnaider): Implement better `grow` and `shrink` methods.
// TODO(adsnaider): Allow cloning the allocator. This might be tricky in an async environment right
// now since we can get interrupts during the allocation/deallocation/etc., so we will need better
// safety guarantees.
//
// SAFTEY: We hopefully implemented the allocator correctly. In particular,
//
// * Memory blocks returned point to valid blocks since creating the allocator and extending it
//   provide ownership of the memory chunks as a precondition. Additionally, the implementation
//   guarantees that no allocated block will be reallocated without being freed first.
// * We don't have a method for cloning the allocator.
// * Allocated pointers may be passed to the `grow`, `shrink`, and `deallocate` methods safely.
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
                    Self::insert_after(node, next);
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
                Self::insert_after(prev.as_mut(), node);
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
        let (pre, node, leftover) = unsafe { memory_region.reinterpret_aligned()? };
        log::debug!("Wrote head sentinel. Wasted {} bytes", pre.len());
        let head = node.write(Node::sentinel());
        let (pre, node, leftover) = unsafe { leftover.reinterpret_aligned()? };
        let tail = node.write(Node::sentinel());
        log::debug!("Wrote tail sentinel. Wasted {} bytes", pre.len());

        let (pre, node) = unsafe { Node::claim_region(leftover)? };
        log::debug!("Wrote intial buffer node. Wasted {} bytes", pre.len());

        Node::link(head, node);
        Node::link(node, tail);

        Some(LinkedListAllocator {
            head: head.into(),
            tail: tail.into(),
            coverage: memory_region,
        })
    }

    unsafe fn extend(&mut self, size: usize) -> Result<(), ExtendError> {
        let new_region = MemoryRegion::from_addr_and_size(self.coverage.end(), size);
        if new_region.wraps() {
            return Err(ExtendError::WouldWrap);
        }
        if let Some(mut last) = unsafe { self.tail.as_ref().prev() } {
            unsafe {
                if last.as_ref().buffer().end() == self.coverage().end() {
                    last.as_mut().grow(size)
                }
            }
        }
        unsafe {
            let (_pad, last) = Node::claim_region(new_region).ok_or(ExtendError::Insufficient)?;
            self.insert_last(last);
        }
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

    #[test]
    fn multiple_alloc_and_frees() {
        init_logging();
        let arena = Arena::new(4096);
        let alloc = unsafe { LinkedListAllocator::from_region(arena.region()) }.unwrap();

        for i in 0..1024 {
            let mut v: Vec<u32, _> = Vec::new_in(&alloc);

            for x in 0..u32::min(i, 256) {
                v.push(x);
            }

            for (i, val) in v.iter().copied().enumerate() {
                assert_eq!(i as u32, val);
            }
        }
    }

    // TODO(adsnaider): Fuzz tests.
}
