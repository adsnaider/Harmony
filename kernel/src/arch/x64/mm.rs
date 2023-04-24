//! Memory management.

use bootloader_api::info::MemoryRegions;
use x86_64::structures::paging::{FrameAllocator, OffsetPageTable, PhysFrame};
use x86_64::VirtAddr;

use self::paging::PHYSICAL_MEMORY_OFFSET;
pub(super) use crate::arch::mm::frames::FRAME_ALLOCATOR;
use crate::arch::mm::paging::PAGE_MAPPER;

mod frames;
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
