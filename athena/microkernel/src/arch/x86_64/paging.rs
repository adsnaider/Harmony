use x86_64_impl::structures::paging::PhysFrame;
use x86_64_impl::PhysAddr;

use crate::PMO;

pub const PAGE_SIZE: usize = 4096;

/// A physical frame that should only be used at boot time.
pub struct RawFrame {
    phys_address: u64,
}

impl RawFrame {
    pub fn from_start_address(address: u64) -> Self {
        Self {
            phys_address: address,
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
