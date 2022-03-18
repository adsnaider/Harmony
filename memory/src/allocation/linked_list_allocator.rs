//! Allocator that manages free memory with a linked list.

mod node;

use core::alloc::{AllocError, Allocator, Layout};
use core::ptr::NonNull;

use self::node::Node;
use super::{ExtendError, MemoryRegion, MemoryRegionAllocator};

/// A type of allocator that uses a linked list to manage free memory blocks.
#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub struct LinkedListAllocator {
    head: NonNull<Node>,
    tail: NonNull<Node>,
    coverage: MemoryRegion,
}

unsafe impl Allocator for LinkedListAllocator {
    fn allocate(&self, _layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        todo!();
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        todo!();
    }
}

unsafe impl MemoryRegionAllocator for LinkedListAllocator {
    unsafe fn from_region(memory_region: MemoryRegion) -> Option<Self> {
        // SAFETY: We are passed ownership of the memory region.
        let node = unsafe { Node::claim_region(memory_region)? };
        Some(LinkedListAllocator {
            head: node.into(),
            tail: node.into(),
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
