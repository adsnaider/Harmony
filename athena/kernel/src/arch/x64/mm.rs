//! Memory management.

use bootloader_api::info::MemoryRegions;
pub use x86_64::structures::paging::{Page, PageSize};
use x86_64::VirtAddr;

use self::paging::PAGE_MAPPER;
use crate::arch::mm::frames::FRAME_ALLOCATOR;

pub mod frames;
pub mod paging;

pub use self::frames::Frame;
pub use self::paging::VirtPage;

mod heap;

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
