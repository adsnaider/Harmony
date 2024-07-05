use core::sync::atomic::{AtomicU64, Ordering};

use x86_64_impl::registers::control::Cr3;
pub use x86_64_impl::structures::paging::PageTableFlags;

use super::{Page, PhysAddr, RawFrame, VirtAddr};
use crate::bump_allocator::BumpAllocator;
use crate::kptr::KPtr;
use crate::retyping::RetypeError;

#[repr(transparent)]
pub struct Addrspace<'a>(&'a AnyPageTable);

#[derive(Debug)]
pub enum MapperError {
    FrameAllocationError,
    HugeParentEntry,
    AlreadyMapped(RawFrame),
}

impl<'a> Addrspace<'a> {
    /// Constructs a manipulable Addrspace from the l4 Frame
    ///
    /// # Safety
    ///
    /// The provided frame must be an l4 frame for some page table addressing.
    pub unsafe fn from_frame(l4_frame: RawFrame) -> Self {
        let table = l4_frame.base().to_virtual().as_ptr();
        Self(unsafe { &*table })
    }

    pub fn l4_frame(&self) -> RawFrame {
        RawFrame::from_start_address(unsafe {
            VirtAddr::from_ptr(self.0 as *const _).to_physical()
        })
    }

    /// Constructs a manipulable Addrspace from the top level page table.
    ///
    /// # Safety
    ///
    /// The provided table must be the root (L4) table.
    pub unsafe fn from_table(table: &'a AnyPageTable) -> Self {
        Self(table)
    }

    // FIXME: Huge pages
    /// Recursively finds the mapping for a page to a frame.
    pub fn get(&self, page: Page) -> Option<(RawFrame, PageTableFlags)> {
        let mut table = self.0;
        let idx = page.base().p4_index();
        let (frame, _) = table.get(idx).get()?;

        table = unsafe { &*frame.base().to_virtual().as_ptr() };
        let idx = page.base().p3_index();
        let (frame, _) = table.get(idx).get()?;

        table = unsafe { &*frame.base().to_virtual().as_ptr() };
        let idx = page.base().p2_index();
        let (frame, _) = table.get(idx).get()?;

        table = unsafe { &*frame.base().to_virtual().as_ptr() };
        let idx = page.base().p1_index();
        let (frame, flags) = table.get(idx).get()?;

        Some((frame, flags))
    }

    /// Maps a virtual page to a physical frame.
    ///
    /// # Safety
    ///
    /// Creating virtual memory mappings is a fundamentally unsafe operation as it enables
    /// aliasing (shared memory).
    pub unsafe fn map_to(
        &self,
        page: Page,
        frame: RawFrame,
        flags: PageTableFlags,
        parent_flags: PageTableFlags,
        frame_allocator: &mut BumpAllocator,
    ) -> Result<(), MapperError> {
        let mut level = Some(PageTableLevel::top());
        let mut table = self.0;
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
                            .alloc_kernel_frame()
                            .ok_or(MapperError::FrameAllocationError)?
                            .into_raw();
                        let addr: *mut AnyPageTable = frame.base().to_virtual().as_mut_ptr();
                        addr.write(AnyPageTable::new());
                        table = unsafe { &*addr };
                        entry.set(frame, parent_flags | PageTableFlags::PRESENT);
                    }
                }
            }
        }
        Ok(())
    }
}

#[repr(C, align(4096))]
pub struct AnyPageTable([PageTableEntry; 512]);

impl Default for AnyPageTable {
    fn default() -> Self {
        Self::new()
    }
}

impl AnyPageTable {
    pub const fn new() -> Self {
        // SAFETY: This is correct for a page table
        unsafe { core::mem::zeroed() }
    }

    /// Returns the addrspace for this page table, enabling memory manipulations
    ///
    /// # Safety
    ///
    /// This must be a root-level page table.
    pub unsafe fn as_addrspace(&self) -> Addrspace<'_> {
        unsafe { Addrspace::from_table(self) }
    }

    pub fn current() -> KPtr<Self> {
        let frame = Self::current_raw();
        unsafe { KPtr::from_frame_unchecked(frame.try_as_kernel().unwrap()) }
    }

    pub fn current_raw() -> RawFrame {
        let (frame, _flags) = Cr3::read();
        RawFrame::from_start_address(PhysAddr::new(frame.start_address().as_u64()))
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
                    log::debug!("Mapping kernel map: {frame:?}, {flags:?}");
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

    /// Atomically sets the frame and attributes on the page table offset provided
    ///
    /// # Safety
    ///
    /// This is one of those methods that fundamentally change memory and can cause undefined
    /// behaviour even when the usage is semantically reasonable.
    pub unsafe fn map(
        &self,
        offset: PageTableOffset,
        frame: RawFrame,
        attributes: PageTableFlags,
    ) -> Option<(RawFrame, PageTableFlags)> {
        self.get(offset).set(frame, attributes)
    }

    /// Atomically unamps the entry (leaving it available for use again).
    ///
    /// # Safety
    ///
    /// This is one of those methods that fundamentally change memory and can cause undefined
    /// behaviour even when the usage is semantically reasonable.
    pub unsafe fn unmap(&self, offset: PageTableOffset) -> Option<(RawFrame, PageTableFlags)> {
        self.get(offset).reset()
    }

    /// Atomically sets the flags on the provided entry.
    ///
    /// # Notes
    ///
    /// The operations themselves are atomic, however, there's no guarantee that another
    /// thread hasn't modified the frame.
    ///
    /// # Safety
    ///
    /// This is one of those methods that fundamentally change memory and can cause undefined
    /// behaviour even when the usage is semantically reasonable.
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

impl Default for PageTableEntry {
    fn default() -> Self {
        Self::new()
    }
}

impl PageTableEntry {
    const FRAME_MASK: u64 = 0x000F_FFFF_FFFF_F000;
    const FLAGS_MASK: u64 = !Self::FRAME_MASK;

    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    pub fn get(&self) -> Option<(RawFrame, PageTableFlags)> {
        let value = self.0.load(Ordering::Relaxed);
        if value == 0 {
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

        if old == 0 {
            return None;
        }
        let addr = old & Self::FRAME_MASK;
        let attributes = PageTableFlags::from_bits(old & Self::FLAGS_MASK).unwrap();
        Some((
            RawFrame::from_start_address(PhysAddr::new(addr)),
            attributes,
        ))
    }

    /// Atomically sets this entry to the frame and the attributes
    ///
    /// # Safety
    ///
    /// This could fundamentally change memory, leading to unsoundness.
    pub unsafe fn set(
        &self,
        frame: RawFrame,
        attributes: PageTableFlags,
    ) -> Option<(RawFrame, PageTableFlags)> {
        unsafe { self.set_bits(attributes.bits() | frame.base().as_u64()) }
    }

    /// Atomically unsets this entry, leaving it empty
    ///
    /// # Safety
    ///
    /// This could fundamentally change memory, leading to unsoundness.
    pub unsafe fn reset(&self) -> Option<(RawFrame, PageTableFlags)> {
        unsafe { self.set_bits(0) }
    }

    /// Atomically sets the flags on this entry (leaving the frame unchanged)
    ///
    /// # Safety
    ///
    /// This could fundamentally change memory, leading to unsoundness.
    pub unsafe fn set_flags(&self, flags: PageTableFlags) -> PageTableFlags {
        let old = self
            .0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some((value & Self::FRAME_MASK) | flags.bits())
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
