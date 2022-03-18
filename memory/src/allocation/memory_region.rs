//! A region in virtual memory space.

use core::ops::Range;

/// The struct represents a region in virtual memory space.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct MemoryRegion {
    addr: *mut u8,
    size: usize,
}

impl MemoryRegion {
    /// Constructs a MemoryRegion from the range.
    pub fn from_ptr_range(range: Range<*mut u8>) -> Self {
        MemoryRegion {
            addr: range.start,
            size: range.end as usize - range.start as usize,
        }
    }

    /// Returns the start pointer in the range.
    pub fn start(&self) -> *mut u8 {
        self.addr
    }

    /// Returns the end pointer in the range (exclusive).
    pub fn end(&self) -> *mut u8 {
        self.addr.wrapping_add(self.size)
    }

    /// Returns the length in bytes of the range.
    pub fn len(&self) -> usize {
        self.size
    }

    /// Returns whether the range is empty (i.e. it's length is 0).
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}
