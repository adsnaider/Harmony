//! x86-64-specifc code and constructs.
use bootloader_api::info::MemoryRegions;
use critical_section::CriticalSection;

pub mod execution;
pub mod inst;
pub mod interrupts;
pub mod mm;

pub use gdt::{sysret, PRIVILEGE_STACK_ADDR};

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
pub unsafe fn init(cs: CriticalSection, memory_map: &'static mut MemoryRegions) {
    gdt::init();
    log::info!("Initialized the Global Decriptor Table");
    interrupts::init(cs);
    log::info!("Initialized interrupts and handlers");

    execution::init();

    mm::retyping::init(memory_map);
    // SAFETY: We only construct a PIT here.
    unsafe {
        timer::Pit8253::steal().into_timer(5966);
    }
}
