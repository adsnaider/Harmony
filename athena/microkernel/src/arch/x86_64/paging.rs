use x86_64_impl::structures::paging::PhysFrame;
use x86_64_impl::PhysAddr;

use crate::kptr::KPtr;
use crate::retyping::KernelFrame;
use crate::PMO;

pub const PAGE_SIZE: usize = 4096;

/// A physical frame that should only be used at boot time.
#[derive(Clone, Copy, Debug)]
pub struct RawFrame {
    phys_address: u64,
}

impl RawFrame {
    pub fn from_start_address(address: u64) -> Self {
        Self {
            phys_address: address,
        }
    }

    pub fn from_index(idx: usize) -> Self {
        Self {
            phys_address: idx as u64 * PAGE_SIZE as u64,
        }
    }

    pub fn index(&self) -> usize {
        self.phys_address as usize / PAGE_SIZE
    }

    /// Returns the raw frame for the sepecific PMO pointer.
    ///
    /// # Safety
    ///
    /// The pointer passed must have been created from [`RawFrame::as_ptr`] or [`RawFrame::as_ptr_mut`]
    pub unsafe fn from_ptr<T>(addr: *mut T) -> Self {
        let addr = addr as usize;
        assert!(addr as usize % PAGE_SIZE == 0);
        Self {
            phys_address: (addr - *PMO) as u64,
        }
    }

    /// This assumes identity mapping.
    pub fn as_ptr<T>(&self) -> *const T {
        (self.phys_address + *PMO as u64) as *const T
    }

    pub fn as_ptr_mut<T>(&self) -> *mut T {
        (self.phys_address + *PMO as u64) as *mut T
    }

    pub(super) fn into_phys_frame(self) -> PhysFrame {
        PhysFrame::from_start_address(PhysAddr::new(self.phys_address)).unwrap()
    }
}

#[repr(transparent)]
pub struct PageTable(x86_64_impl::structures::paging::PageTable);

impl PageTable {
    pub unsafe fn from_l4_frame(frame: KernelFrame<'static>) -> KPtr<Self> {
        unsafe { KPtr::from_frame_unchecked(frame) }
    }
}
