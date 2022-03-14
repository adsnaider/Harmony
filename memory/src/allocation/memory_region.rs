//! A region in virtual memory space.

/// The struct represents a region in virtual memory space.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct MemoryRegion {
    addr: *mut u8,
    size: usize,
}
