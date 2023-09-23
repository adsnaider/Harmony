//! Virtual memory management.
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU64, Ordering};

use critical_section::CriticalSection;
use thiserror::Error;
use x86_64::addr::VirtAddrNotValid;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::mapper::{MapToError, UnmapError};
use x86_64::structures::paging::page::AddressNotAligned;
use x86_64::structures::paging::{Mapper, OffsetPageTable, Page, PageTable, Size4KiB, Translate};
use x86_64::{PhysAddr, VirtAddr};

use super::frames::{Frame, FRAME_ALLOCATOR};

/// Flags for page mapping.
pub type PageTableFlags = x86_64::structures::paging::PageTableFlags;

pub(super) static PHYSICAL_MEMORY_OFFSET: VirtAddr = {
    // SAFETY: Address is canonical.
    unsafe { VirtAddr::new_unsafe(0xFFFF_F000_0000_0000) }
};

pub(super) fn init(pmo: VirtAddr, _cs: CriticalSection) {
    assert_eq!(pmo, PHYSICAL_MEMORY_OFFSET);
    let l4_table = {
        let (frame, _) = Cr3::read();
        let virt = pmo + frame.start_address().as_u64();
        // SAFETY: This is valid since the PageTable is initialized in the cr3 and the physical
        // memory offset must be correct.
        unsafe { &mut *(virt.as_u64() as *mut PageTable) }
    };

    // SAFETY: We get the l4_table provided by the bootloader which maps the memory to
    // `pmo`.
    let page_map = unsafe { OffsetPageTable::new(l4_table, pmo) };

    // Sanity check, let's check some small addresses, should be mapped to themselves.
    assert!(page_map.translate_addr(pmo + 0x0u64) == Some(PhysAddr::new(0)));
    assert!(page_map.translate_addr(pmo + 0xABCDu64) == Some(PhysAddr::new(0xABCD)));
    assert!(page_map.translate_addr(pmo + 0xABAB_0000u64) == Some(PhysAddr::new(0xABAB_0000)));
}

/// An isolated virtual memory space.
#[derive(Debug, Eq, PartialEq)]
pub struct AddrSpace {
    l4_frame: Frame,
}

impl AddrSpace {
    /// Creates a virtual space.
    ///
    /// The virtual space will only include the kernel pages.
    ///
    /// Note that kernel pages are not user-accessible (i.e. from Ring 3).
    pub fn new() -> Option<Self> {
        let l4_frame = Frame::alloc()?;
        let l4_table: &mut MaybeUninit<PageTable> =
            unsafe { &mut *l4_frame.physical_offset().as_mut_ptr() };

        let l4_table = l4_table.write(PageTable::new());
        let current_table = AddrSpace::current().l4_table().clone();
        for i in 256..512 {
            l4_table[i] = current_table[i].clone();
        }
        // SAFETY: Table is initialized and memory is exclusively allocated.
        Some(Self { l4_frame })
    }

    /// Returns the current virtual address space.
    pub fn current() -> Self {
        AddrSpace {
            l4_frame: Cr3::read().0.into(),
        }
    }

    /// Activates this address space, returning the one previously active.
    ///
    /// # Safety
    ///
    /// Obvious perils of changing memory spaces.
    pub unsafe fn activate(&self) -> Self {
        let (old_frame, flags) = Cr3::read();
        let old_frame = old_frame.into();
        if self.l4_frame != old_frame {
            // SAFETY: Precondition.
            unsafe { Cr3::write(self.l4_frame.into(), flags) }
        }
        Self {
            l4_frame: old_frame,
        }
    }

    /// Maps a virtual page to a physical frame.
    ///
    /// Note that a page must be unmapped before it can be remapped.
    ///
    /// # Safety
    ///
    /// You are fundamentally changing memory any time this function is called.
    pub unsafe fn map_to(
        &mut self,
        page: VirtPage,
        frame: Frame,
        flags: PageTableFlags,
    ) -> Result<(), MapToError<Size4KiB>> {
        unsafe {
            critical_section::with(|cs| {
                self.page_table()
                    .map_to(
                        page.into(),
                        frame.into(),
                        flags,
                        &mut *FRAME_ALLOCATOR.lock(cs),
                    )
                    .map(|map_flush| map_flush.flush())
            })
        }
    }

    /// Unmaps the given virtual page from the frame.
    ///
    /// # Safety
    ///
    /// You are fundamentally changing memory.
    pub unsafe fn unmap(&mut self, page: VirtPage) -> Result<Frame, UnmapError> {
        let mut page_table = self.page_table();
        page_table.unmap(page.into()).map(|(frame, flush)| {
            flush.flush();
            frame.into()
        })
    }

    /// Translates the given virtual address to the mapped physical address.
    pub fn translate(&self, addr: u64) -> Result<Option<u64>, VirtAddrNotValid> {
        Ok(self
            .page_table()
            .translate_addr(VirtAddr::try_new(addr)?)
            .map(|addr| addr.as_u64()))
    }

    fn l4_table(&self) -> &PageTable {
        unsafe { &*self.l4_frame.physical_offset().as_ptr() }
    }

    fn l4_table_mut(&self) -> &mut PageTable {
        unsafe { &mut *self.l4_frame.physical_offset().as_mut_ptr() }
    }

    fn page_table(&self) -> OffsetPageTable {
        unsafe { OffsetPageTable::new(self.l4_table_mut(), PHYSICAL_MEMORY_OFFSET) }
    }
}

/// A virtual-space Page.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[repr(transparent)]
pub struct VirtPage(Page<Size4KiB>);

impl From<Page<Size4KiB>> for VirtPage {
    fn from(value: Page<Size4KiB>) -> Self {
        Self(value)
    }
}

impl From<VirtPage> for Page<Size4KiB> {
    fn from(value: VirtPage) -> Self {
        value.0
    }
}

/// Attempted to construct an invalid page.
#[derive(Debug, Error)]
pub enum InvalidPage {
    #[error("The page is not aligned to a page boundary")]
    /// The page is not aligned to a page boundary.
    NotAligned,
    #[error("The provided virtual address is invalid")]
    /// The address is not canonical.
    InvalidAddress,
}

impl From<AddressNotAligned> for InvalidPage {
    fn from(_value: AddressNotAligned) -> Self {
        Self::NotAligned
    }
}
impl From<VirtAddrNotValid> for InvalidPage {
    fn from(_value: VirtAddrNotValid) -> Self {
        Self::InvalidAddress
    }
}

impl VirtPage {
    /// Constructs a age from the provided virtual address.
    pub fn from_start_address(addr: u64) -> Result<Self, InvalidPage> {
        Ok(Page::from_start_address(VirtAddr::try_new(addr)?)?.into())
    }

    /// Returns a pointer to the start of the page.
    pub fn as_ptr<T>(&self) -> *const T {
        self.0.start_address().as_ptr()
    }

    /// Returns a mutable pointer to the start of the page.
    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.0.start_address().as_mut_ptr()
    }

    /// Allocates a new frame and maps it to some available page.
    pub fn alloc() -> Option<Self> {
        // FIXME: Need a buddy allocator or something here.
        static PAGE_OFFSET: AtomicU64 = AtomicU64::new(0xFFFF_8800_0000_0000);
        let frame = Frame::alloc()?;
        let start_addr = PAGE_OFFSET.fetch_add(4096, Ordering::Relaxed);
        // SAFETY: The virtual address and frame are both unique.
        unsafe {
            let page: VirtPage = Page::from_start_address(VirtAddr::new(start_addr))
                .unwrap()
                .into();
            AddrSpace::current()
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                )
                .unwrap();
            Some(page)
        }
    }

    /// Returns the starting address as a u64.
    pub fn start_address(&self) -> u64 {
        self.0.start_address().as_u64()
    }

    /// Returns the size of the page.
    pub fn size(&self) -> u64 {
        self.0.size()
    }
}
