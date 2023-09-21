//! Virtual memory management.
use core::sync::atomic::{AtomicU64, Ordering};

use critical_section::CriticalSection;
use singleton::Singleton;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::mapper::{MapToError, UnmapError};
use x86_64::structures::paging::{
    Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, Size4KiB, Translate,
};
use x86_64::{PhysAddr, VirtAddr};

use super::frames::{Frame, FRAME_ALLOCATOR};

pub(super) static PAGE_MAPPER: Singleton<OffsetPageTable<'static>> = Singleton::uninit();

pub(super) static PHYSICAL_MEMORY_OFFSET: VirtAddr = {
    // SAFETY: Address is canonical.
    unsafe { VirtAddr::new_unsafe(0xFFFF_F000_0000_0000) }
};

pub(super) fn init(pmo: VirtAddr, cs: CriticalSection) {
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

    PAGE_MAPPER.initialize(page_map, cs);
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
        let l4_page = l4_frame.physical_offset();

        let mut l4_table = PageTable::new();
        let current_table = AddrSpace::current();
        let current_page_table = current_table.l4_table();
        for i in 256..512 {
            l4_table[i] = current_page_table[i].clone();
        }
        // SAFETY: Table is initialized and memory is exclusively allocated.
        unsafe {
            core::ptr::write(l4_page.as_mut_ptr(), l4_table);
        }
        Some(Self { l4_frame })
    }

    fn l4_table(&self) -> &PageTable {
        unsafe { *self.l4_frame.physical_offset().as_ptr() }
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

impl VirtPage {
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
            page.map_to(frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE)
                .unwrap();
            Some(page)
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
        self,
        frame: Frame,
        flags: PageTableFlags,
    ) -> Result<(), MapToError<Size4KiB>> {
        unsafe {
            critical_section::with(|cs| {
                PAGE_MAPPER
                    .lock(cs)
                    .map_to(
                        self.into(),
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
    pub unsafe fn unmap(self) -> Result<Frame, UnmapError> {
        critical_section::with(|cs| PAGE_MAPPER.lock(cs).unmap(self.into())).map(
            |(frame, flush)| {
                flush.flush();
                frame.into()
            },
        )
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
