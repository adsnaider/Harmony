//! Functionality on individual nodes.

use core::alloc::Layout;
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

pub enum SplitNodeResult {
    Hijack,
    Partition(usize),
    Misfit,
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
    pub unsafe fn claim_region<'a>(region: MemoryRegion) -> Option<(MemoryRegion, &'a mut Self)> {
        // SAFETY: We are gauaranteed that the can be mutably aliased.
        let (pre, node, buffer) = unsafe { region.reinterpret_aligned()? };
        Some((pre, node.write(Node::new(buffer))))
    }

    /// Returns the `next` pointer.
    pub fn next(&self) -> Option<NonNull<Self>> {
        self.next
    }

    pub fn set_next(&mut self, next: Option<NonNull<Node>>) {
        self.next = next;
    }

    /// Returns the `prev` pointer.
    pub fn prev(&self) -> Option<NonNull<Self>> {
        self.prev
    }

    pub fn buffer(&self) -> MemoryRegion {
        self.buffer
    }

    /// Returns true if the node can be used to allocate the `layout`.
    pub fn fits(&self, layout: Layout) -> bool {
        self.buffer.is_aligned(layout.align()) && self.buffer.len() >= layout.size()
    }

    pub fn shrink_to(&mut self, size: usize) -> Option<MemoryRegion> {
        let leftover;
        (self.buffer, leftover) = self.buffer.partition(size)?;
        Some(leftover)
    }

    pub fn grow(&mut self, by: usize) {
        self.buffer = MemoryRegion::from_addr_and_size(self.buffer.start(), self.buffer.len() + by);
    }

    pub fn split_for_layout(&self, layout: Layout) -> SplitNodeResult {
        if !self.buffer.is_aligned(layout.align()) {
            return SplitNodeResult::Misfit;
        }
        match self
            .buffer
            .aligned_at(core::mem::align_of::<Node>(), layout.size())
        {
            None => SplitNodeResult::Misfit,
            Some((data, leftover)) => {
                if leftover.len() <= core::mem::size_of::<Node>() {
                    SplitNodeResult::Hijack
                } else {
                    SplitNodeResult::Partition(data.len())
                }
            }
        }
    }

    pub fn link(left: &mut Self, right: &mut Self) {
        left.next = Some(right.into());
        right.prev = Some(left.into());
    }
}
