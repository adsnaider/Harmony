use core::sync::atomic::{AtomicU64, Ordering};

use x86_64_impl::structures::paging::PageTableFlags;

use super::RawFrame;

pub const TOP_TABLE: u8 = 4;

const EXISTS_BIT: PageTableFlags = PageTableFlags::BIT_9;
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

    pub const fn is_lower_half(&self) -> bool {
        self.0 < 256
    }
}

#[repr(C, align(4096))]
pub struct AnyPageTable {
    entries: [PageTableEntry; ENTRY_COUNT],
}

impl AnyPageTable {
    pub const fn new() -> Self {
        // SAFETY: This is correct for a page table
        unsafe { core::mem::zeroed() }
    }

    fn get(&self, offset: PageTableOffset) -> &PageTableEntry {
        unsafe { self.entries.get_unchecked(offset.0) }
    }

    pub unsafe fn map(
        &self,
        offset: PageTableOffset,
        frame: RawFrame,
        attributes: PageTableFlags,
    ) -> Option<(RawFrame, PageTableFlags)> {
        self.get(offset).set(frame, attributes)
    }

    pub unsafe fn unmap(&self, offset: PageTableOffset) -> Option<(RawFrame, PageTableFlags)> {
        self.get(offset).reset()
    }

    pub unsafe fn set_flags(
        &self,
        offset: PageTableOffset,
        attributes: PageTableFlags,
    ) -> PageTableFlags {
        self.get(offset).set_flags(attributes)
    }
}

#[repr(transparent)]
pub struct PageTableEntry(AtomicU64);

impl PageTableEntry {
    const FRAME_MASK: u64 = 0x000F_FFFF_FFFF_F000;
    const FLAGS_MASK: u64 = !Self::FRAME_MASK;

    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    pub fn get(&self) -> Option<(RawFrame, PageTableFlags)> {
        let value = self.0.load(Ordering::Relaxed);
        if value & EXISTS_BIT.bits() == 0 {
            return None;
        }
        let frame = RawFrame::from_start_address(value & Self::FRAME_MASK);
        let flags = PageTableFlags::from_bits(value & Self::FLAGS_MASK).unwrap();
        Some((frame, flags))
    }

    pub fn frame(&self) -> Option<RawFrame> {
        self.get().map(|x| x.0)
    }

    pub fn flags(&self) -> Option<PageTableFlags> {
        self.get().map(|x| x.1)
    }

    unsafe fn set_bits(&self, bits: u64) -> Option<(RawFrame, PageTableFlags)> {
        let old = self.0.swap(bits, Ordering::Relaxed);

        if old & EXISTS_BIT.bits() == 0 {
            return None;
        }
        let addr = old & Self::FRAME_MASK;
        let attributes = PageTableFlags::from_bits(old & Self::FLAGS_MASK).unwrap();
        Some((RawFrame::from_start_address(addr), attributes))
    }

    pub unsafe fn set(
        &self,
        frame: RawFrame,
        attributes: PageTableFlags,
    ) -> Option<(RawFrame, PageTableFlags)> {
        unsafe { self.set_bits(EXISTS_BIT.bits() | attributes.bits() | frame.phys_address) }
    }

    pub unsafe fn reset(&self) -> Option<(RawFrame, PageTableFlags)> {
        unsafe { self.set_bits(0) }
    }

    pub unsafe fn set_flags(&self, flags: PageTableFlags) -> PageTableFlags {
        let old = self
            .0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some((value & Self::FRAME_MASK) | EXISTS_BIT.bits() | flags.bits())
            })
            .unwrap();

        PageTableFlags::from_bits(old & Self::FLAGS_MASK).unwrap()
    }
}
