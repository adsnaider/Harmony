//! x86-64-specifc code and constructs.
use bootloader_api::info::MemoryRegions;
use critical_section::CriticalSection;

pub mod context;
pub mod inst;
pub mod interrupts;
pub mod mm;

mod gdt;
mod timer;

/// Initialize the system.
///
/// # Safety
///
/// The memory offset and memory map must both be accurately representing the full span of memory.
///
/// # Panics
///
/// If `init` is called more than once.
pub unsafe fn init(
    physical_memory_offset: u64,
    memory_map: &mut MemoryRegions,
    cs: CriticalSection,
) {
    // SAFETY: Precondition.
    unsafe {
        mm::init(physical_memory_offset, memory_map);
    }
    log::info!("Initialized memory manager");
    gdt::init();
    log::info!("Initialized the Global Decriptor Table");
    interrupts::init(cs);
    log::info!("Initialized interrupts and handlers");

    context::init();

    // SAFETY: We only construct a PIT here.
    unsafe {
        timer::Pit8253::new().into_timer(5966);
    }
}
