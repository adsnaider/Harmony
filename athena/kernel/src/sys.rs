//! System management and functionality.

use bootloader_api::info::MemoryRegion;
use bootloader_api::BootInfo;
use critical_section::CriticalSection;

use crate::arch::mm::frames::PHYSICAL_MEMORY_OFFSET;

pub mod serial;

/// System intialization routine.
///
/// Sets up the logger, memory utilities, interrupts, and architecture-specific
/// constructs.
///
/// # Safety
///
/// The information in `bootinfo` must be accurate.
pub(super) unsafe fn init(bootinfo: &'static mut BootInfo, cs: CriticalSection) {
    // SAFETY: Bootloader passed the framebuffer correctly.
    serial::init();
    log::info!("Hello, logging!");

    log::debug!(
        "Memory map starts at {:#?}",
        &*bootinfo.memory_regions as *const [MemoryRegion]
    );
    let pmo = bootinfo
        .physical_memory_offset
        .into_option()
        .expect("No memory offset found from bootloader.");
    log::debug!("Physical memory offset is {:#?}", pmo as *const ());
    assert_eq!(
        pmo,
        PHYSICAL_MEMORY_OFFSET.as_u64(),
        "Physical offset not where it was expected"
    );

    // SAFETY: The physical memory offset is correct, well-aligned, and canonical, and the memory
    // map is correct from the bootloader.
    unsafe { crate::arch::init(cs, &mut bootinfo.memory_regions) }
}
