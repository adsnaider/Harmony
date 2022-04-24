//! A memory page.

use core::marker::PhantomData;

use super::{PageSize, Size4KiB};

/// A memory page.
///
/// A page defines the virtual address that can be used within code to access memory. However, this
/// struct doesn't guarantee that the addresses it points to will be mapped to a frame or what data
/// that frame would have.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Page<S: PageSize = Size4KiB> {
    /// The virtual start of the page. This address should be aligned to `S::SIZE`.
    virt_start: usize,
    /// Phantom.
    _phantom: PhantomData<S>,
}

impl<S: PageSize> Page<S> {
    /// Constructs a new page.
    ///
    /// # Panics
    ///
    /// If `virt_start` isn't aligned to `S::SIZE`
    pub fn new(virt_start: usize) -> Self {
        if virt_start % S::SIZE != 0 {
            panic!("Frame should always be aligned to the boundary.");
        }
        // SAFETY: Checked that `virt_start` is correctly aligned.
        unsafe { Self::new_unchecked(virt_start) }
    }

    /// Constructs a new page.
    ///
    /// # Safety
    ///
    /// `phys_start` must be aligned to `S::SIZE`.
    pub unsafe fn new_unchecked(virt_start: usize) -> Self {
        Self {
            virt_start,
            _phantom: PhantomData,
        }
    }

    /// Returns the virtual starting address of the page.
    pub fn virt_start(&self) -> *mut u8 {
        self.virt_start as *mut u8
    }
}
