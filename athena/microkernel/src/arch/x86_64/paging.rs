use x86_64_impl::structures::paging::PhysFrame;
use x86_64_impl::PhysAddr;

pub use self::page_table::PageTable;
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

pub mod page_table {
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
    }

    impl PageTableEntry {
        pub const fn new() -> Self {
            Self(AtomicU64::new(0))
        }
    }
}
