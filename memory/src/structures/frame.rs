//! The physical memory frames.

use core::marker::PhantomData;

use super::{PageSize, Size4KiB};

/// A physical memory frame.
///
/// Frames represent the direct mapping to physical memory. In general, frames aren't addressable
/// since they are reached upon by the TLB's address conversion. Instead, a `[Page]` is the
/// addressable entity which will map 1-1 with the frames.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Frame<S: PageSize = Size4KiB> {
    /// The physical start of the page. This address should always be aligned to `S::SIZE`.
    phys_start: usize,
    /// Phantom.
    _phantom: PhantomData<S>,
}

impl<S: PageSize> Frame<S> {
    /// Constructs a new frame.
    ///
    /// # Panics
    ///
    /// If `phys_start` isn't aligned to `S::SIZE`
    pub fn new(phys_start: usize) -> Self {
        if phys_start % S::SIZE != 0 {
            panic!("Frame should always be aligned to the boundary.");
        }
        // SAFETY: Checked that `phys_start` is correctly aligned.
        unsafe { Self::new_unchecked(phys_start) }
    }

    /// Constructs a new frame.
    ///
    /// # Safety
    ///
    /// `phys_start` must be aligned to `S::SIZE`.
    pub unsafe fn new_unchecked(phys_start: usize) -> Self {
        Self {
            phys_start,
            _phantom: PhantomData,
        }
    }

    /// Returns the physical starting address of the frame.
    pub fn phys_start(&self) -> usize {
        self.phys_start
    }
}
