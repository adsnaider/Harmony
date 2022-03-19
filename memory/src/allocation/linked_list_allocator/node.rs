//! Functionality on individual nodes.

use core::alloc::Layout;
use core::ptr::NonNull;

use crate::allocation::MemoryRegion;

/// A node in a linked list for the free list allocator.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct Node {
    /// Next pointer.
    next: Option<NonNull<Node>>,
    /// Previous pointer.
    prev: Option<NonNull<Node>>,
    /// Buffer managed by this `Node`.
    buffer: MemoryRegion,
}

// TODO(adsnaider): Statically check that the Node's alignment + the size is aligned to 8 bytes.

/// The result of splitting a node for a layout.
pub enum SplitNodeResult {
    /// Hijack the node, taking all the data with it.
    Hijack,
    /// Partition the node at `.0` before using for allocation.
    Partition(usize),
    /// Don't use the node for allocation.
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

    /// Constructs a sentinel node that has null empty buffer.
    pub fn sentinel() -> Self {
        Self {
            next: None,
            prev: None,
            buffer: MemoryRegion::from_addr_and_size(core::ptr::null_mut(), 0),
        }
    }

    /// Returns true if the node is a sentinel.
    pub fn is_sentinel(&self) -> bool {
        self.buffer.start().is_null() && self.buffer.is_empty()
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

    /// Sets the `next` pointer.
    pub fn set_next(&mut self, next: Option<NonNull<Node>>) {
        self.next = next;
    }

    /// Returns the `prev` pointer.
    pub fn prev(&self) -> Option<NonNull<Self>> {
        self.prev
    }

    /// Sets the `prev` pointer.
    pub fn set_prev(&mut self, prev: Option<NonNull<Node>>) {
        self.prev = prev;
    }

    /// Returns the `buffer` corresponding to this node.
    pub fn buffer(&self) -> MemoryRegion {
        self.buffer
    }

    /// Shrinks the buffer to `size` and returns the leftover region. If `size` is larger than the
    /// current buffer, it returns None and the buffer won't change.
    pub fn shrink_to(&mut self, size: usize) -> Option<MemoryRegion> {
        let leftover;
        (self.buffer, leftover) = self.buffer.partition(size)?;
        Some(leftover)
    }

    /// Grows the buffer by `count` bytes.
    pub fn grow(&mut self, count: usize) {
        self.buffer =
            MemoryRegion::from_addr_and_size(self.buffer.start(), self.buffer.len() + count);
    }

    /// Returns the `SplitNodeResult` for the given layout.
    ///
    /// You can think of this function as a hypothetical of what would happen if we tried to use
    /// this node to allocate for a given layout. The result can either be a `Misfit` (alignment or
    /// size constraints), a `Partition(at)` which indicates that the node should be shrunk to the
    /// returned size, or a `Hijack`, which indicates that the node should be used to its entierty
    /// to allocate the layout.
    ///
    /// The caller may assume that on a `Partition` result, the provided partition point will be
    /// correct to allow for a new Node to be created on the leftover space and that the node will
    /// have a non-empty buffer.
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

    /// Doubly links two nodes together.
    pub fn link(left: &mut Self, right: &mut Self) {
        left.next = Some(right.into());
        right.prev = Some(left.into());
    }
}
