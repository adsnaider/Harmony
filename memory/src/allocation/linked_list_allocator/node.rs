//! Functionality on individual nodes.

use core::ptr::NonNull;

use crate::allocation::MemoryRegion;

/// A node in a linked list for the free list allocator.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct Node {
    next: Option<NonNull<Node>>,
    prev: Option<NonNull<Node>>,
    buffer: MemoryRegion,
}

impl Node {
    /// Constructs a new `Node` that manages the `region`.
    pub fn new(region: MemoryRegion) -> Self {
        Self {
            next: None,
            prev: None,
            buffer: region,
        }
    }

    /// Constructs a node, hijacking the region for self storage.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that we can create a mutable reference within the region.
    pub unsafe fn claim_region<'a>(region: MemoryRegion) -> Option<&'a mut Self> {
        let (_pre, node, buffer) = unsafe { region.reinterpret_aligned()? };
        Some(node.write(Node::new(buffer)))
    }
}
