//! Allocator that manages free memory with a linked list.

mod node;

use core::alloc::{AllocError, Allocator, Layout};
use core::marker::PhantomData;
use core::ops::Range;
use core::ptr::NonNull;

use self::node::{Node, SplitNodeResult};
use super::{ExtendError, MemoryRegion, MemoryRegionAllocator};

// TODO(adsnaider): Rust is unclear what the pointer offset arithmetic allowed operations are. In
// particular, it's undefined behavior to have pointers offset 1 element past the original
// allocated object. In the context of an allocator, it's unclear what that means though as memory
// is directly provided to the allocator by the operating system and the allocator can sort of
// extend itself when the OS says it can.
/// A type of allocator that uses a linked list to manage free memory blocks.
///
/// There's a safety invariant associated with the structure. The linked list is said to be
/// well-structured when:
///
/// * The head and tail nodes are sentinel nodes at the beginning of the coverage region.
/// * All nodes in the list form a doubly linked list, where no cycles exist (all nodes are
///   distinct).
/// * The head is the start node and has no prev pointer.
/// * The tail is the end node and has no next pointer.
/// * The memory regions covered by every node don't overlap with any other node's and their
///   covered regions.
/// * The nodes are linked in order by their pointers (with the exception of the head and tail
///   nodes which are at the beginning of the coverage region).
/// * No nodes in the list are currently allocated.
///
/// Many functions may list the above invariant in their safety documentation, meaning that the
/// caller must guarantee that the list is well-structured when calling the function.
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
    /// * No nodes can be mutably aliased during the iteration steps, though they can be in between
    ///   steps.
    /// * The structure of the linked list also shouldn't change between iteration.
    /// * The list must be well-structured.
    unsafe fn iter(&self) -> Iter<'_> {
        Iter {
            // SAFETY: No nodes may be mutably aliased during iteration.
            current: unsafe { self.head.as_ref().next().unwrap() },
            _phantom: PhantomData,
        }
    }

    /// Insets the node `next` after the node `prev` in the linked list, correctly linking their
    /// neighbors.
    ///
    /// The resulting list will be well-structured if it was originally well-structured and `next`
    /// is not allocated.
    ///
    /// # Safety
    ///
    /// * The list must be well-structured.
    /// * No references exist into the linked list other that `prev` and `next`.
    /// * `next` must not be part of the list before calling this function.
    ///
    /// # Panics
    ///
    /// If the node inserted causes the list to get out of order.
    unsafe fn insert_after(prev: &mut Node, next: &mut Node) {
        if !prev.is_sentinel() && prev.buffer().start() > next.buffer().start() {
            panic!(
                "Inserting {:#?} after {:#?} would cause the list to get out of order.",
                next, prev
            );
        }
        // SAFETY: No references exist into the linked list and because the list is will strutured
        // and `next` is not in it, then prev->next is not aliased.
        let after = unsafe { prev.next().unwrap().as_mut() };
        if !after.is_sentinel() && next.buffer().start() > after.buffer().start() {
            panic!(
                "Inserting {:#?} after {:#?} would cause the list to get out of order because the following node {:#?} should be behind.",
                next, prev, after);
        }
        Node::link(next, after);
        Node::link(prev, next);
    }

    /// Inserts a node to the end of the list (right before the `tail` sentinel).
    ///
    /// The resulting list will be well-structured if `node` isn't allocated.
    ///
    /// # Safety
    ///
    /// * No references can exist to any nodes in the list during the function call.
    /// * The list must be well-structured.
    /// * `node` must not be part of the list or the sentinels.
    ///
    /// # Panics
    ///
    /// If the node inserted causes the list to get out of order.
    unsafe fn insert_last(&self, node: &mut Node) {
        // SAFETY: No references exist other than `node` which isn't part of the list.
        // Additionally, the list is well-structured.
        unsafe {
            Self::insert_after(self.tail.as_ref().prev().unwrap().as_mut(), node);
        }
    }

    /// Inserts a node to the beginning of the list (right after the `head` sentinel).
    ///
    /// The resulting list will be well-structured if `node` isn't allocated.
    ///
    /// # Safety
    ///
    /// * No references can exist to any nodes in the list during the function call.
    /// * The list must be well-structured.
    /// * `node` must not be part of the list or the sentinels.
    ///
    /// # Panics
    ///
    /// If the node inserted causes the list to get out of order.
    unsafe fn insert_first(&self, node: &mut Node) {
        // SAFETY: No references exist other than `node` which isn't part of the list.
        // Additionally, the list is well-structured.
        unsafe {
            Self::insert_after(&mut *self.head.as_ptr(), node);
        }
    }

    /// Unlinks a node from the list.
    ///
    /// This method causes `node`'s neighbors to skip through `node`. Additionally, the method will
    /// clear out the links from `node`.
    ///
    /// The resulting list will be well-structured.
    ///
    /// # Safety
    ///
    /// * No references must exist to any nodes other than `node` in the list.
    /// * The list must be well-structured.
    ///
    /// # Panics
    ///
    /// If the node is a `sentinel` node.
    unsafe fn unlink_node(&self, node: &mut Node) {
        // SAFETY: no references exist and because the list is well-structured, node->prev and
        // node->next ar valid and distinct nodes.
        Node::link(unsafe { node.prev().unwrap().as_mut() }, unsafe {
            node.next().unwrap().as_mut()
        });

        node.set_next(None);
        node.set_prev(None);
    }

    /// Coalesces 2 nodes if they are contiguous in memory.
    ///
    /// The resulting list is well-structured if the original list is well-structured.
    ///
    /// # Safety
    ///
    /// * The list must be well-structured.
    /// * After the function returns, if the result is Some, then the returned node can be
    ///   derreferenced but neither one of the parameters should be dereferenced.
    /// * No references can exist into the nodes in the list.
    /// * `left` and `right` must be true left to right neighbors.
    ///
    /// # Panics
    ///
    /// If either `left` or `right` are the sentinel nodes.
    unsafe fn coalesce_neighbors(
        left: NonNull<Node>,
        right: NonNull<Node>,
    ) -> Option<NonNull<Node>> {
        // SAFETY: no references exist into the list.
        if unsafe { left.as_ref() }.buffer().end() as usize == right.as_ptr() as usize {
            // SAFETY: no references exist into the list.
            let chunk = MemoryRegion::from_ptr_range(Range {
                start: left.as_ptr() as *mut u8,
                end: unsafe { right.as_ref() }.buffer().end(),
            });
            // SAFETY: No references exist into the list and all `left`, `right`, `left_neighbor`,
            // and `right_neighbor are distinct because the list is well-structured.
            let (left_neighbor, right_neighbor) = unsafe {
                (
                    left.as_ref().prev().unwrap().as_mut(),
                    right.as_ref().next().unwrap().as_mut(),
                )
            };

            // SAFETY: We don't use `left` or `right anymore. Their memory is freed and create a
            // new "supernode" in that region. Because the list is well-structured, we know memory
            // is well maintained and the order is maintained.
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
    /// * `node` is a valid node in the list.
    /// * The list is well-structured.
    ///
    /// # Panics
    ///
    /// If `node` is either sentinel node.
    unsafe fn coalesce(&self, mut node: NonNull<Node>) -> NonNull<Node> {
        // SAFETY: No references exist and `node` is valid.
        unsafe {
            log::debug!("Coallescing node {:#?}", node.as_ref());
        }
        // SAFETY: No references exist and `node` is valid.
        let prev = unsafe { node.as_ref().prev().unwrap() };
        // SAFETY: No references exist and prev is distinct from `node` since the list is
        // well-structured.
        if unsafe { !prev.as_ref().is_sentinel() } {
            // SAFETY:
            //  * List is well-structured
            //  * We overwrite the original node and don't use `prev` if the returned value is
            //  Some.
            //  * No references exist into the list (All references are dead).
            //  * `prev` and `node` are neighbors.
            unsafe {
                if let Some(n) = Self::coalesce_neighbors(prev, node) {
                    node = n;
                }
            }
        }
        // SAFETY: No references exist. `node` is valid because we updated it after coalescing.
        let next = unsafe { node.as_ref().next().unwrap() };
        // SAFETY: No references exist and `next` is distinct from node because the list is
        // well-structured.
        if unsafe { !next.as_ref().is_sentinel() } {
            // SAFETY:
            //  * List is still well-structured
            //  * We overwrite the original node and don't use `prev` if the returned value is
            //    Some.
            //  * No references exist into the list (All references are dead).
            //  * `node` and `next` are neighbors.
            unsafe {
                if let Some(n) = Self::coalesce_neighbors(node, next) {
                    node = n;
                }
            }
        }

        // SAFETY: `node` is valid because we've updated it and no references exist into the list.
        unsafe {
            log::debug!("Final node is {:#?}", node.as_ref());
        }
        node
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
        // SAFETY: Precondition of `iter` method guarantees that no mutable references exist into
        // the list during the iteration step.
        if unsafe { self.current.as_ref().is_sentinel() } {
            return None;
        }
        let current = Some(self.current);
        // SAFETY: Precondition of `iter` method guarantees that no mutable references exist into
        // the list during the iteration step.
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
        // SAFETY: We iterate and don't have any mutable references during the iteration. The list
        // is maintained. The list is well-structured.
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

        // SAFETY: We don't have any more references into the list and iteration is done.
        let node = unsafe { node.as_mut() };

        log::info!("Found suitable node: {:#?}", node);

        match split {
            SplitNodeResult::Hijack => {
                log::debug!("Hijacking node.")
            }
            SplitNodeResult::Partition(at) => {
                log::debug!("Partition node at {}", at);
                let remainder = node.shrink_to(at).unwrap();
                // SAFETY: We've shrunk the previous node, so the region is completely managed by
                // the new Node. This works because the list is well-structured.
                let (pre, next) = unsafe { Node::claim_region(remainder) }.unwrap();
                debug_assert!(
                    pre.is_empty(),
                    "Partition split should return the split with correct padding."
                );
                debug_assert!(
                    !next.buffer().is_empty(),
                    "Partition split could be hijacked."
                );

                // SAFETY:
                // * The list is originally well-structured.
                // * The only references are `node` and `next`.
                unsafe {
                    Self::insert_after(node, next);
                    // NOTE: List is still well-structured.
                }
            }
            SplitNodeResult::Misfit => {
                panic!("Shouldn't get here as we found a node with a different split.");
            }
        }

        // SAFETY: Only reference alive is `node`. The list is still well-structured since it's
        // guaranteed by the `insert_after` method.
        unsafe {
            self.unlink_node(node);
            // NOTE: List is still well-structured.
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
        // SAFETY: For a `ptr` to be allocated, there must have been a `node` right behind it (with
        // no padding), since otherwise, the allocation would have been a misfit. No references
        // exist into the list at this point.
        // TODO(adsnaider): Does ptr and the node belong to the same allocation? Rust's raw pointer
        // manipulation is a bit wonky when it comes to pointer offsets, so this may not be
        // allowed.
        let node = unsafe { &mut *(ptr.as_ptr().sub(core::mem::size_of::<Node>()) as *mut Node) };

        log::info!("Deallocating node {:#?}", node);

        let mut prev = None;
        // SAFETY: The only mutable reference alive is `node`. Notice that `node` can't be in the
        // list since it has been allocated.
        for candidate in unsafe { self.iter() } {
            if ptr.as_ptr() < candidate.as_ptr() as *mut u8 {
                break;
            }
            prev = Some(candidate);
        }
        if let Some(mut prev) = prev {
            // SAFETY: prev hasn't been dereferenced. Must be distnct from `node` since `node`
            // wasn't on the list to begin with. Additionally, the list is well-structured.
            unsafe {
                log::debug!("Linking node after {:#?}", prev.as_ref(),);
                Self::insert_after(prev.as_mut(), node);
                // Note: List is still well-structured.
            }
        } else {
            // SAFETY: All references are dead (except for node), the list is well structured since
            // we haven't changed the list, and `node` isn't on the list.
            unsafe {
                self.insert_first(node);
                // NOTE: List is still well-structured.
            }
        }
        // NOTE: We inserted node into the list because we have deallocated the data. This
        // maintains the structure of the list.

        // SAFETY: All references are dead, we have inserted `node` in the list and the list is
        // still well-structured.
        unsafe {
            self.coalesce(node.into());
        }
    }
}

// SAFETY: The allocator's coverage is correct.
unsafe impl MemoryRegionAllocator for LinkedListAllocator {
    unsafe fn from_region(memory_region: MemoryRegion) -> Option<Self> {
        // SAFETY: We are passed ownership of the memory region.
        let (pre, node, leftover) = unsafe { memory_region.reinterpret_aligned()? };
        log::debug!("Wrote head sentinel. Wasted {} bytes", pre.len());
        let head = node.write(Node::sentinel());
        // SAFETY: We still have ownership of `leftover`.
        let (pre, node, leftover) = unsafe { leftover.reinterpret_aligned()? };
        let tail = node.write(Node::sentinel());
        log::debug!("Wrote tail sentinel. Wasted {} bytes", pre.len());

        // SAFETY: We stil have ownership of leftover.
        let (pre, node) = unsafe { Node::claim_region(leftover)? };
        log::debug!("Wrote intial buffer node. Wasted {} bytes", pre.len());

        Node::link(head, node);
        Node::link(node, tail);
        // NOTE: The list is well-structured at this point:
        // * head and tail are sentinels
        // * The nodes form a doubly linked list.
        // * The head is at the start and tail is at the end
        // * There's no overlap of memory regions.
        // * The structure in memory is [HEAD|TAIL|NODE w/ buffer|]
        // * All the nodes are deallocated.

        // NOTE: We initialize the coverage to the original memory region.
        Some(LinkedListAllocator {
            head: head.into(),
            tail: tail.into(),
            coverage: memory_region,
        })
    }

    unsafe fn extend(&mut self, size: usize) -> Result<(), ExtendError> {
        let new_region = MemoryRegion::from_addr_and_size(self.coverage.end(), size);
        // Check for wrapping errors.
        if new_region.wraps() {
            return Err(ExtendError::WouldWrap);
        }
        // SAFETY: We don't have any other references into the list.
        let last = unsafe { self.tail.as_ref().prev().unwrap().as_mut() };
        // Check to see if we can extend the tail.
        if !last.is_sentinel() && last.buffer().end() == self.coverage().end() {
            // SAFETY: We don't have any references into the list other than `last`.
            // Additionally, we can assume ownership of the region so, in this case, the
            // ownership is give to the `last` node.
            last.grow(size)
            // NOTE: The list is still well-structured here as we only extend the last node
            // (so no overlapping), and only do so if the the end of its range matches the end
            // of coverage. This guarantees that there aren't any nodes or allocated space that
            // is getting invalidated by growing the node.
        } else {
            // We couldn't extend the last block because either there is no last or because the
            // node isn't contiguous with the extra memory region. Regardless, we crate a new
            // node and insert it to the end of the list.

            // SAFETY: We use a new node to take possession of the memory region. This is fine
            // because we take ownership of the region.
            unsafe {
                let (_pad, last) =
                    Node::claim_region(new_region).ok_or(ExtendError::Insufficient)?;
                self.insert_last(last);
                // NOTE: The list is still well-structured here as insert_last guarantees so.
            }
        }
        self.coverage =
            MemoryRegion::from_addr_and_size(self.coverage.start(), self.coverage.len() + size);
        // NOTE: We update the coverage to be accurate.
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
