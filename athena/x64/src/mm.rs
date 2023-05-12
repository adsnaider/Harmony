//! Memory management.

use core::sync::atomic::Ordering::Relaxed;
use core::sync::atomic::{AtomicU64, AtomicUsize};

use bootloader_api::info::MemoryRegions;
use singleton::Singleton;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags, PhysFrame,
};
use x86_64::VirtAddr;

use self::frames::FRAME_ALLOCATOR;
use self::paging::{PAGE_MAPPER, PHYSICAL_MEMORY_OFFSET};

pub(crate) mod frames;
mod heap;
mod paging;

/// Initializes the system memory management. Allocation calls can be made after `init`.
///
/// # Safety
///
/// The physical_memory_offset must be in accordance to the currently setup page table and the
/// memory map must represent memory accurately.
///
/// # Panics
///
/// If called more than once.
pub(super) unsafe fn init(physical_memory_offset: u64, memory_map: &mut MemoryRegions) {
    let physical_memory_offset = VirtAddr::new(physical_memory_offset);
    critical_section::with(|cs| {
        frames::init(physical_memory_offset, memory_map, cs);
        log::info!("Initialized frame allocator.");

        paging::init(physical_memory_offset, cs);
        log::info!("Page mapper initialized");

        heap::init(
            &mut *PAGE_MAPPER.lock(cs),
            &mut *FRAME_ALLOCATOR.lock(cs),
            cs,
        );
        log::info!("Allocator initialized");
    })
}

/// Returns an offset page table that can be used with a new context.
///
/// The l4 page is returned as well and the lifetime of the page table is mapped to that
///
/// # Safety
///
/// Lifetime of the table is tied to the frame returned
/// all shannanigans involved with modifying the virtual memory space.
pub(super) unsafe fn make_new_page_table<'a>() -> Option<(OffsetPageTable<'a>, PhysFrame)> {
    critical_section::with(|cs| {
        let l4_table = paging::dup_page_table();
        let l4_frame = FRAME_ALLOCATOR.locked(cs, |allocator| allocator.allocate_frame())?;

        let l4_addr = PHYSICAL_MEMORY_OFFSET + l4_frame.start_address().as_u64();

        unsafe {
            core::ptr::write(l4_addr.as_mut_ptr(), l4_table);
            let offset_table =
                OffsetPageTable::new(&mut *l4_addr.as_mut_ptr(), PHYSICAL_MEMORY_OFFSET);
            Some((offset_table, l4_frame))
        }
    })
}

/// Allocates a frame and maps it to an available page.
pub fn alloc_page() -> Option<Page> {
    static PAGE_OFFSET: AtomicU64 = AtomicU64::new(0xFFFF_8800_0000_0000);
    critical_section::with(|cs| {
        let mut frame_allocator = FRAME_ALLOCATOR.lock(cs);
        let frame = frame_allocator.allocate_frame()?;
        let start_addr = PAGE_OFFSET.fetch_add(4096, Relaxed);
        unsafe {
            let page = Page::from_start_address_unchecked(VirtAddr::new_unsafe(start_addr));
            PAGE_MAPPER.locked(cs, |map| {
                map.map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    &mut *frame_allocator,
                )
                .unwrap()
                .flush();
            });
            Some(page)
        }
    })
}
