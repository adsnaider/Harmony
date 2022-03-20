//! A region in virtual memory space.

use core::mem::MaybeUninit;
use core::ops::Range;

/// The struct represents a region in virtual memory space.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct MemoryRegion {
    /// Starting address of the region.
    addr: *mut u8,
    /// Size in bytes covered by the region.
    size: usize,
}

impl MemoryRegion {
    /// Constructs a MemoryRegion from the range.
    pub fn from_ptr_range(range: Range<*mut u8>) -> Self {
        Self {
            addr: range.start,
            size: range.end as usize - range.start as usize,
        }
    }

    /// Returns true if the memory region wraps through the virtual memory space.
    pub fn wraps(&self) -> bool {
        (self.addr as usize).overflowing_add(self.size - 1).1
    }

    /// Constructs a MemoryRegion with the range [addr, addr + size)
    pub fn from_addr_and_size(addr: *mut u8, size: usize) -> Self {
        Self { addr, size }
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

    /// Partitions the region into two contiguous chunks at `at` bytes.
    ///
    /// Returns None if `at > self.len()`
    pub fn partition(&self, at: usize) -> Option<(Self, Self)> {
        if at > self.len() {
            None
        } else {
            Some((
                MemoryRegion::from_addr_and_size(self.start(), at),
                MemoryRegion::from_addr_and_size(self.start().wrapping_add(at), self.size - at),
            ))
        }
    }

    /// Returns true when the start of the region is aligned to `alignemnt`.
    pub fn is_aligned(&self, alignment: usize) -> bool {
        (alignment - ((self.addr as usize) % alignment)) % alignment == 0
    }

    /// Returns an aligned copy of the memory region and the pre-padding leftover.
    pub fn aligned(&self, alignment: usize) -> Option<(Self, Self)> {
        let offset = (alignment - ((self.addr as usize) % alignment)) % alignment;
        self.partition(offset)
    }

    /// Returns the first possible partition at or after `hint` that is aligned to `alignment`.
    pub fn aligned_at(&self, alignment: usize, hint: usize) -> Option<(Self, Self)> {
        let offset = (alignment - (self.addr as usize + hint) % alignment) % alignment;
        self.partition(hint + offset)
    }

    /// Same as `aligned()` but uses the appropriate alignment for `T`.
    pub fn aligned_for<T>(&self) -> Option<(Self, Self)> {
        self.aligned(core::mem::align_of::<T>())
    }

    /// Reinterprets the beginning of the region as `MaybeUninit<T>`, applying the necessary
    /// alignment and returning the initial padding and the leftover bytes as well.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that we can create a mutable reference within the region.
    pub unsafe fn reinterpret_aligned<'a, T>(
        &self,
    ) -> Option<(Self, &'a mut MaybeUninit<T>, Self)> {
        let (pre, buffer) = self.aligned_for::<T>()?;
        let (data, post) = buffer.partition(core::mem::size_of::<T>())?;
        // SAFETY: Caller guarantees that the region can be reinterpreted. Additionally, we have
        // correct alignment and size because we checked so.
        let data = unsafe { &mut *(data.addr as *mut MaybeUninit<T>) };
        Some((pre, data, post))
    }
}
