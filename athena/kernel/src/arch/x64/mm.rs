//! Memory management.

use bootloader_api::info::MemoryRegions;
use x86_64::VirtAddr;

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

        heap::init(cs);
        log::info!("Allocator initialized");
    })
}
