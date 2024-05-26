use core::sync::atomic::{AtomicU64, Ordering};

use x86_64_impl::registers::control::Cr3;
pub use x86_64_impl::structures::paging::PageTableFlags;

use super::{Page, PhysAddr, RawFrame};
use crate::bump_allocator::BumpAllocator;
use crate::kptr::KPtr;
use crate::retyping::RetypeError;

const EXISTS_BIT: PageTableFlags = PageTableFlags::BIT_9;

#[repr(transparent)]
pub struct Addrspace(KPtr<AnyPageTable>);

#[derive(Debug)]
pub enum MapperError {
    FrameAllocationError,
    HugeParentEntry,
    AlreadyMapped(RawFrame),
}

impl Addrspace {
    pub fn new(frame: RawFrame) -> Result<Self, RetypeError> {
        Ok(Self(AnyPageTable::new_l4(frame)?))
    }

    pub unsafe fn map_to(
        &self,
        page: Page,
        frame: RawFrame,
        flags: PageTableFlags,
        frame_allocator: &mut BumpAllocator,
    ) -> Result<(), MapperError> {
        let mut level = Some(PageTableLevel::top());
        let mut table = &*self.0;
        let addr = page.base();
        while let Some(current_level) = level {
            level = current_level.lower();
            let offset = addr.page_table_index(current_level);
            let entry = table.get(offset);
            match entry.get() {
                Some((frame, flags)) => {
                    if current_level.level() == 1 {
                        return Err(MapperError::AlreadyMapped(frame));
                    }
                    if flags.contains(PageTableFlags::HUGE_PAGE) {
                        return Err(MapperError::HugeParentEntry);
                    }
                    table = unsafe { &*frame.base().to_virtual().as_ptr() };
                }
                None => {
                    if current_level.is_bottom() {
                        entry.set(frame, flags);
                    } else {
                        let frame = frame_allocator
                            .alloc_frame()
                            .ok_or(MapperError::FrameAllocationError)?;
                        let addr: *mut AnyPageTable = frame.base().to_virtual().as_mut_ptr();
                        addr.write(AnyPageTable::new());
                        table = unsafe { &*addr };
                        entry.set(
                            frame,
                            PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

#[repr(C, align(4096))]
pub struct AnyPageTable([PageTableEntry; 512]);

impl AnyPageTable {
    pub const fn new() -> Self {
        // SAFETY: This is correct for a page table
        unsafe { core::mem::zeroed() }
    }

    pub fn current() -> KPtr<Self> {
        let (frame, _flags) = Cr3::read();
        let frame = RawFrame::from_start_address(PhysAddr::new(frame.start_address().as_u64()));
        unsafe { KPtr::from_frame_unchecked(frame.try_as_kernel().unwrap()) }
    }

    pub fn new_l4(frame: RawFrame) -> Result<KPtr<Self>, RetypeError> {
        KPtr::new(frame, AnyPageTable::clone_kernel())
    }

    pub fn clone_kernel() -> Self {
        let current = AnyPageTable::current();

        let new = Self::new();
        for i in 256..512 {
            let offset = PageTableOffset::new(i).unwrap();
            unsafe {
                if let Some((frame, flags)) = current.get(offset).get() {
                    new.map(offset, frame, flags);
                }
            }
        }
        new
    }

    pub fn get(&self, offset: PageTableOffset) -> &PageTableEntry {
        // SAFETY: Offset is within [0, 512)
        unsafe { self.0.get_unchecked(offset.0 as usize) }
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
        let frame = RawFrame::from_start_address(PhysAddr::new(value & Self::FRAME_MASK));
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
        Some((
            RawFrame::from_start_address(PhysAddr::new(addr)),
            attributes,
        ))
    }

    pub unsafe fn set(
        &self,
        frame: RawFrame,
        attributes: PageTableFlags,
    ) -> Option<(RawFrame, PageTableFlags)> {
        unsafe { self.set_bits(EXISTS_BIT.bits() | attributes.bits() | frame.base().as_u64()) }
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

#[derive(Debug, Copy, Clone)]
pub struct PageTableOffset(u16);

#[derive(Debug, Copy, Clone)]
pub struct PageTableLevel(u8);

#[derive(Debug)]
pub struct InvalidLevel;

impl PageTableLevel {
    pub const fn new(level: u8) -> Self {
        match Self::try_new(level) {
            Ok(level) => level,
            Err(_) => panic!("Page table level must be within 1 and 4",),
        }
    }

    pub const fn try_new(level: u8) -> Result<Self, InvalidLevel> {
        if level < 1 || level > 4 {
            return Err(InvalidLevel);
        }
        Ok(Self(level))
    }

    pub const fn level(&self) -> u8 {
        self.0
    }

    pub const fn top() -> Self {
        Self(4)
    }

    pub const fn is_bottom(&self) -> bool {
        self.level() == 1
    }

    pub const fn lower(self) -> Option<Self> {
        match Self::try_new(self.level() - 1) {
            Ok(l) => Some(l),
            Err(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum PageTableOffsetError {
    OutOfBounds,
}

impl TryFrom<u16> for PageTableOffset {
    type Error = PageTableOffsetError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<usize> for PageTableOffset {
    type Error = PageTableOffsetError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(u16::try_from(value).map_err(|_| PageTableOffsetError::OutOfBounds)?)
    }
}

impl PageTableOffset {
    pub const fn new(offset: u16) -> Result<Self, PageTableOffsetError> {
        if offset < 512 {
            Ok(Self(offset))
        } else {
            Err(PageTableOffsetError::OutOfBounds)
        }
    }

    pub const fn is_lower_half(&self) -> bool {
        self.0 < 256
    }

    pub const fn new_truncate(addr: u16) -> Self {
        Self(addr % 512)
    }
}
