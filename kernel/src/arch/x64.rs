//! x86-64 architecture-dependent code.

use bootloader_api::info::MemoryRegions;

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

struct SingleThreadCS();
critical_section::set_impl!(SingleThreadCS);
/// SAFETY: While the OS kernel is running in a single thread, then disabling interrupts is a safe
/// to guarantee a critical section's conditions.
unsafe impl critical_section::Impl for SingleThreadCS {
    unsafe fn acquire() -> critical_section::RawRestoreState {
        let interrupts_enabled = x86_64::instructions::interrupts::are_enabled();
        x86_64::instructions::interrupts::disable();
        interrupts_enabled
    }

    unsafe fn release(interrupts_were_enabled: critical_section::RawRestoreState) {
        if interrupts_were_enabled {
            x86_64::instructions::interrupts::enable();
        }
    }
}
