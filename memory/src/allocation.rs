//! Memory allocators.

use core::alloc::Allocator;

pub mod linked_list_allocator;
pub mod memory_region;

pub use linked_list_allocator::LinkedListAllocator;
pub use memory_region::MemoryRegion;

/// Error returned when the allocator couldn't be extended.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ExtendError {
    /// The extension size isn't large enough to self-support the allocator.
    Insufficient,
    /// The extension would cause the allocator's coverage to wrap.
    WouldWrap,
}

/// Allocator that works on an extensible `MemoryRegion`.
///
/// # Safety
///
/// The allocator must correctly coverage.
pub unsafe trait MemoryRegionAllocator: Allocator + Sized {
    /// Attempts to construct the allocator from the provided region.
    ///
    /// # Returns
    ///
    /// The allocator if the region is able to self-support the allocator.
    ///
    /// # Safety
    ///
    /// If the function returns `Option::None`, then it's safe to utilize the memory region after
    /// the call, otherwise, no references that overlap the memory region must exist for the
    /// lifetime of the allocator.
    unsafe fn from_region(memory_region: MemoryRegion) -> Option<Self>;
    /// Attempts to extend the allocator's coverage by `size` bytes.
    ///
    /// Returns whether the new bytes are able to be used by the allocator. If the result is an
    /// error, the coverage shouldn't have changed.
    ///
    /// # Safety
    ///
    /// It's up to the caller to guarantee that the resulting coverage won't be aliased and can
    /// safely be managed by the allocator.
    unsafe fn extend(&mut self, size: usize) -> Result<(), ExtendError>;
    /// Get the current coverage of the allocator.
    ///
    /// The coverage is defined as the `MemoryRegion` that the allocator manages which may be less
    /// than the total free/potentially free memory.
    fn coverage(&self) -> MemoryRegion;
}
