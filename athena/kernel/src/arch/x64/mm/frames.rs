//! Physical frame allocation and management.

use thiserror::Error;
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

use super::retyping::{RetypeError, UntypedFrame};
use crate::arch::PAGE_SIZE;

pub static PHYSICAL_MEMORY_OFFSET: VirtAddr = {
    // SAFETY: Address is canonical.
    unsafe { VirtAddr::new_unsafe(0xFFFF_F000_0000_0000) }
};

/// A physical frame.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[repr(transparent)]
pub struct RawFrame(PhysFrame<Size4KiB>);

impl From<PhysFrame<Size4KiB>> for RawFrame {
    fn from(value: PhysFrame<Size4KiB>) -> Self {
        Self(value)
    }
}

impl From<RawFrame> for PhysFrame<Size4KiB> {
    fn from(value: RawFrame) -> Self {
        value.0
    }
}

pub struct FrameBumpAllocator {
    index: usize,
}

#[derive(Error, Debug)]
pub enum AllocError {
    #[error("No more usuable frames available in main memory")]
    OutOfMemory,
}

impl FrameBumpAllocator {
    pub fn new() -> Self {
        Self { index: 0 }
    }

    pub fn alloc_frame(&mut self) -> Result<UntypedFrame<'static>, AllocError> {
        let start = self.index * PAGE_SIZE;
        let frame = loop {
            let frame = RawFrame::from_index(self.index).into_untyped();
            self.index += 1;
            match frame {
                Ok(frame) => break frame,
                Err(RetypeError::OutOfBounds) => return Err(AllocError::OutOfMemory),
                Err(_) => {}
            }
        };
        Ok(frame)
    }
}

unsafe impl FrameAllocator<Size4KiB> for FrameBumpAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.alloc_frame()
            .ok()
            .map(|frame| frame.into_kernel().into_raw().into())
    }
}

impl RawFrame {
    pub const fn size() -> usize {
        4096
    }

    pub const fn align() -> usize {
        4096
    }

    pub fn as_ptr<T>(&self) -> *const T {
        (PHYSICAL_MEMORY_OFFSET + self.0.start_address().as_u64()).as_ptr()
    }

    pub fn as_ptr_mut<T>(&self) -> *mut T {
        (PHYSICAL_MEMORY_OFFSET + self.0.start_address().as_u64()).as_mut_ptr()
    }

    pub fn index(&self) -> usize {
        self.0.start_address().as_u64() as usize / Self::size()
    }

    pub fn from_index(idx: usize) -> Self {
        let start_address = (Self::size() * idx) as u64;
        Self(PhysFrame::from_start_address(PhysAddr::new(start_address)).unwrap())
    }
}
