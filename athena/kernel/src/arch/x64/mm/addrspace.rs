//! User-level memory virtualization.
use core::mem::MaybeUninit;

use critical_section::CriticalSection;
use x86_64::addr::VirtAddrNotValid;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::mapper::{MapToError, UnmapError};
use x86_64::structures::paging::page::AddressNotAligned;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, Size4KiB, Translate,
};
use x86_64::{PhysAddr, VirtAddr};

use super::frames::Frame;

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
#[repr(transparent)]
pub struct AddrSpace {
    l4_frame: Frame,
}

impl AddrSpace {
    /// Creates a virtual space.
    ///
    /// The virtual space will only include the kernel pages.
    ///
    /// Note that kernel pages are not user-accessible (i.e. from Ring 3).
    pub fn new(l4_frame: Frame) -> Self {
        let l4_table: &mut MaybeUninit<PageTable> =
            // SAFETY: Size and alignment are valid for MaybeUninit<PageTable>
            unsafe { &mut *l4_frame.as_ptr_mut() };

        let l4_table = l4_table.write(PageTable::new());
        let current_table =
            // SAFETY: critical section and non-reentrant function prevednt mutable aliasing
            critical_section::with(|_cs| unsafe { AddrSpace::current().l4_table().clone() });
        for i in 256..512 {
            l4_table[i] = current_table[i].clone();
        }
        // SAFETY: Table is initialized and memory is exclusively allocated.
        Self { l4_frame }
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
    pub unsafe fn activate(&mut self) -> Self {
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
        allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Result<(), MapToError<Size4KiB>> {
        // SAFETY: Conditions passed to the caller.
        // SAFETY: critical section and non-reentrant function prevednt mutable aliasing
        critical_section::with(|cs| unsafe {
            self.page_table()
                .map_to(page.into(), frame.into(), flags, allocator)
                .map(|map_flush| map_flush.flush())
        })
    }

    /// Unmaps the given virtual page from the frame.
    ///
    /// # Safety
    ///
    /// You are fundamentally changing memory.
    pub unsafe fn unmap(&mut self, page: VirtPage) -> Result<Frame, UnmapError> {
        // SAFETY: critical section and non-reentrant function prevednt mutable aliasing
        critical_section::with(|_cs| unsafe {
            let mut page_table = self.page_table();
            page_table.unmap(page.into()).map(|(frame, flush)| {
                flush.flush();
                frame.into()
            })
        })
    }

    /// Translates the given virtual address to the mapped physical address.
    pub fn translate(&mut self, addr: u64) -> Result<Option<u64>, VirtAddrNotValid> {
        // SAFETY: critical section and non-reentrant function prevednt mutable aliasing
        critical_section::with(|_cs| unsafe {
            Ok(self
                .page_table()
                .translate_addr(VirtAddr::try_new(addr)?)
                .map(|addr| addr.as_u64()))
        })
    }

    /// # Safety
    ///
    /// No other mutable references to the same table can exist
    unsafe fn l4_table(&self) -> &PageTable {
        // SAFETY: This is valid since l4_frame must have the l4_table
        unsafe { &*self.l4_frame.as_ptr() }
    }

    // SAFETY: No other references to the same table can exist
    unsafe fn l4_table_mut(&mut self) -> &mut PageTable {
        // SAFETY: This is valid since l4_frame must have the l4_table
        unsafe { &mut *self.l4_frame.as_ptr_mut() }
    }

    // SAFETY: No other references to the same table can exist
    unsafe fn page_table(&mut self) -> OffsetPageTable {
        // SAFETY: Physical offset and l4 table are correct.
        // Additionally, while the page table is a mutable reference,
        unsafe { OffsetPageTable::new(self.l4_table_mut(), PHYSICAL_MEMORY_OFFSET) }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct VirtPage(Page<Size4KiB>);

impl VirtPage {
    pub fn from_start_address(addr: u64) -> Result<Self, AddressNotAligned> {
        Ok(Self(Page::from_start_address(VirtAddr::new(addr))?))
    }
}

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
