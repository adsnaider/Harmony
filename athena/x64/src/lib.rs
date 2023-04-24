//! x86-64-specifc code and constructs.
#![no_std]
#![feature(allocator_api)]
#![feature(abi_x86_interrupt)]
#![feature(error_in_core)]
#![feature(negative_impls)]
#![feature(const_fn_floating_point_arithmetic)]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks)]

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
