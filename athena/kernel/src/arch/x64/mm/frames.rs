//! Physical frame allocation and management.

use bootloader_api::info::{MemoryRegion, MemoryRegionKind, MemoryRegions};
use thiserror::Error;
use x86_64::structures::paging::{FrameAllocator, PageSize, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

pub static PHYSICAL_MEMORY_OFFSET: VirtAddr = {
    // SAFETY: Address is canonical.
    unsafe { VirtAddr::new_unsafe(0xFFFF_F000_0000_0000) }
};

/// A physical frame.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[repr(transparent)]
pub struct Frame(PhysFrame<Size4KiB>);

impl From<PhysFrame<Size4KiB>> for Frame {
    fn from(value: PhysFrame<Size4KiB>) -> Self {
        Self(value)
    }
}

impl From<Frame> for PhysFrame<Size4KiB> {
    fn from(value: Frame) -> Self {
        value.0
    }
}

/// Returns true if the memory region is generally usable.
fn is_region_usable(region: &MemoryRegion) -> bool {
    matches!(region.kind, MemoryRegionKind::Usable) && region.end > region.start
}

pub struct FrameBumpAllocator<'a> {
    mmap: &'a mut MemoryRegions,
    index: usize,
}

#[derive(Error, Debug)]
pub enum AllocError {
    #[error("No more usuable frames available in main memory")]
    OutOfMemory,
}

impl<'a> FrameBumpAllocator<'a> {
    pub fn new(mmap: &'a mut MemoryRegions) -> Self {
        Self { mmap, index: 0 }
    }

    pub fn alloc_frame(&mut self) -> Result<Frame, AllocError> {
        let (idx, region) = self
            .mmap
            .iter_mut()
            .enumerate()
            .skip(self.index)
            .filter(|(num, region)| is_region_usable(region))
            .next()
            .ok_or(AllocError::OutOfMemory)?;

        self.index = idx;
        let start = region.start;
        region.start += Size4KiB::SIZE;
        assert!(region.start <= region.end);
        Ok(Frame(
            PhysFrame::from_start_address(PhysAddr::new(start))
                .expect("Regions should be 4k aligned"),
        ))
    }
}

unsafe impl FrameAllocator<Size4KiB> for FrameBumpAllocator<'_> {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.alloc_frame().ok().map(|frame| frame.into())
    }
}

impl Frame {
    pub fn as_ptr<T>(&self) -> *const T {
        (PHYSICAL_MEMORY_OFFSET + self.0.start_address().as_u64()).as_ptr()
    }

    pub fn as_ptr_mut<T>(&self) -> *mut T {
        (PHYSICAL_MEMORY_OFFSET + self.0.start_address().as_u64()).as_mut_ptr()
    }
}
