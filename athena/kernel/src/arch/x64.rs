//! x86-64-specifc code and constructs.
use bootloader_api::info::MemoryRegions;

extern crate alloc;

pub mod context;
pub mod inst;
pub mod int;
pub mod mm;

mod gdt;
mod timer;

/// Initialize the system.
pub unsafe fn init(physical_memory_offset: u64, memory_map: &mut MemoryRegions) {
    critical_section::with(|cs| {
        unsafe {
            mm::init(physical_memory_offset, memory_map);
        }
        log::info!("Initialized memory manager");
        gdt::init();
        log::info!("Initialized the Global Decriptor Table");
        int::init(cs);
        log::info!("Initialized interrupts and handlers");

        context::init();

        unsafe {
            timer::Pit8253::new().into_timer(5966);
        }
    })
}
