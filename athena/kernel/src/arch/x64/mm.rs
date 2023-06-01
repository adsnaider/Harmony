//! Memory management.

use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering::Relaxed;

use bootloader_api::info::MemoryRegions;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, PhysFrame};
use x86_64::VirtAddr;

pub use self::frames::FRAME_ALLOCATOR;
use self::paging::PAGE_MAPPER;

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

/// Get's the L4 frame for the currently active page table.
pub fn active_page_table() -> PhysFrame {
    Cr3::read().0
}

/// Set's the currently active page table.
///
/// # Safety
///
/// The new page table should include the full kernel memory map.
pub unsafe fn set_page_table(l4_frame: PhysFrame) -> PhysFrame {
    let (old_frame, flags) = Cr3::read();
    if l4_frame != old_frame {
        unsafe { Cr3::write(l4_frame, flags) }
    }
    old_frame
}
