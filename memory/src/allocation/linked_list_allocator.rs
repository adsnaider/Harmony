//! Allocator that manages free memory with a linked list.

use core::alloc::{AllocError, Allocator, Layout};
use core::ptr::NonNull;

use super::{ExtendError, MemoryRegion, MemoryRegionAllocator};

/// A type of allocator that uses a linked list to manage free memory blocks.
#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub struct LinkedListAllocator {}

unsafe impl Allocator for LinkedListAllocator {
    fn allocate(&self, _layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        todo!();
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        todo!();
    }
}

unsafe impl MemoryRegionAllocator for LinkedListAllocator {
    unsafe fn from_region(_memory_region: MemoryRegion) -> Option<Self> {
        todo!();
    }

    unsafe fn extend(&mut self, _size: usize) -> Result<(), ExtendError> {
        todo!();
    }

    fn coverage(&self) -> MemoryRegion {
        todo!();
    }
}
