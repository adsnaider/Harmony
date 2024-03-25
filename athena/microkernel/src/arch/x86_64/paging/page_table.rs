use core::sync::atomic::{AtomicU64, Ordering};

use x86_64_impl::structures::paging::PageTableFlags;

use super::RawFrame;
use crate::kptr::KPtr;
use crate::retyping::{KernelFrame, UserFrame};

const ENTRY_COUNT: usize = 512;

pub struct PageTableOffset(usize);

pub enum PageTableOffsetError {
    OutOfBounds,
}

impl TryFrom<usize> for PageTableOffset {
    type Error = PageTableOffsetError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl PageTableOffset {
    pub const fn new(offset: usize) -> Result<Self, PageTableOffsetError> {
        if offset < 512 {
            Ok(Self(offset))
        } else {
            Err(PageTableOffsetError::OutOfBounds)
        }
    }
}

#[repr(C, align(4096))]
pub struct RawPageTable {
    entries: [PageTableEntry; ENTRY_COUNT],
}

impl RawPageTable {
    pub const fn new() -> Self {
        // SAFETY: This is correct for a page table
        unsafe { core::mem::zeroed() }
    }

    pub unsafe fn map(
        &self,
        offset: PageTableOffset,
        frame: RawFrame,
        attributes: PageTableFlags,
    ) -> Option<RawFrame> {
        let entry = unsafe { self.entries.get_unchecked(offset.0) };
        let old = entry
            .0
            .swap(attributes.bits() | frame.phys_address, Ordering::Relaxed);
        if old == 0 {
            return None;
        }
        let addr = old & 0x000F_FFFF_FFFF_F000;
        let frame = RawFrame::from_start_address(addr);
        Some(frame)
    }

    pub unsafe fn unmap(&self, offset: PageTableOffset) -> Option<RawFrame> {
        let entry = unsafe { self.entries.get_unchecked(offset.0) };
        let old = entry.0.swap(0, Ordering::Relaxed);
        if old == 0 {
            return None;
        }
        let addr = old & 0x000F_FFFF_FFFF_F000;
        let frame = RawFrame::from_start_address(addr);
        Some(frame)
    }

    pub unsafe fn into_typed_table<const L: u8>(self) -> PageTable<L> {
        // SAFETY: Identical representation
        unsafe { core::mem::transmute(self) }
    }
}

impl KPtr<RawPageTable> {
    pub unsafe fn into_typed_table<const L: u8>(self) -> KPtr<PageTable<L>> {
        // SAFETY: Identical representation
        unsafe { core::mem::transmute(self) }
    }
}

#[repr(transparent)]
pub struct PageTable<const LEVEL: u8>(RawPageTable);

#[repr(transparent)]
pub struct PageTableEntry(AtomicU64);

impl<const LEVEL: u8> PageTable<LEVEL> {
    pub const fn into_raw_table(self) -> RawPageTable {
        // SAFETY: Identical representation
        unsafe { core::mem::transmute(self) }
    }
}

impl<const LEVEL: u8> KPtr<PageTable<LEVEL>> {
    pub const fn into_raw_table(self) -> KPtr<RawPageTable> {
        // SAFETY: Identical representation
        unsafe { core::mem::transmute(self) }
    }
}

impl PageTable<0> {
    pub fn map(
        &self,
        offset: PageTableOffset,
        frame: UserFrame<'static>,
        attributes: PageTableFlags,
    ) -> Option<UserFrame<'static>> {
        let old = unsafe { self.0.map(offset, frame.into_raw(), attributes) }?;
        let frame = unsafe { UserFrame::from_raw(old).unwrap() };
        Some(frame)
    }

    pub fn unmap(&self, offset: PageTableOffset) -> Option<UserFrame<'static>> {
        let old = unsafe { self.0.unmap(offset) }?;
        let frame = unsafe { UserFrame::from_raw(old).unwrap() };
        Some(frame)
    }
}
impl PageTable<4> {
    pub unsafe fn from_l4_frame(frame: KernelFrame<'static>) -> KPtr<Self> {
        unsafe { KPtr::from_frame_unchecked(frame) }
    }

    pub fn translate_page(&self, page: u64) -> Option<RawFrame> {
        let mut table = &self.0;

        let offset = (page >> 39) & 0x1FF;
        todo!();
    }
}

impl PageTableEntry {
    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }
}
