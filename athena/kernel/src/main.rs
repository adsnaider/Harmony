//! Kernel entry and executable. Ideally, this is just a thin wrapper over all of the kernel's
//! components.
#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(error_in_core)]
#![feature(abi_x86_interrupt)]
#![feature(inline_const)]
#![deny(absolute_paths_not_starting_with_crate)]
#![deny(unsafe_op_in_unsafe_fn)]
// #![warn(missing_docs)]
#![warn(clippy::undocumented_unsafe_blocks)]

pub(crate) mod util;

mod sys;

pub mod arch;
pub mod capabilities;
pub mod components;
pub mod proc;
pub mod thread;

use bootloader_api::config::Mapping;
use bootloader_api::{entry_point, BootInfo, BootloaderConfig};
use include_bytes_aligned::include_bytes_aligned;

use crate::arch::mm::frames::FrameBumpAllocator;
use crate::proc::Process;

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    critical_section::with(|_| {
        log::error!("{}", info);
        loop {
            arch::inst::hlt();
        }
    })
}

/// Initializes the system.
///
/// # Safety
///
/// `bootinfo` must be correct.
unsafe fn init(bootinfo: &'static mut BootInfo) {
    crate::arch::interrupts::disable();
    critical_section::with(|cs| {
        // SAFETY: The bootinfo is directly provided by the bootloader.
        unsafe { sys::init(bootinfo, cs) };
    });
}

/// Kernel's starting point.
fn kmain(bootinfo: &'static mut BootInfo) -> ! {
    static INIT: &[u8] = include_bytes_aligned!(8, "../programs/init");

    // SAFETY: bootinfo is correct.
    unsafe {
        init(bootinfo);
    }
    log::info!("System initialization complete");

    let mut fallocator = FrameBumpAllocator::new();
    let mut init = Process::load(INIT, 10, &mut fallocator).unwrap();
    init.exec();
}

const CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.kernel_stack_size = 4 * 1024 * 1024; // 4MiB
    config.mappings.dynamic_range_start = Some(0xFFFF_8000_0000_0000);
    config.mappings.dynamic_range_end = Some(0xFFFF_9000_0000_0000);
    config.mappings.physical_memory = Some(Mapping::FixedAddress(0xFFFF_F000_0000_0000));
    config.mappings.kernel_stack = Mapping::FixedAddress(0xFFFF_EFFF_FFBF_0000);
    config.mappings.boot_info = Mapping::FixedAddress(0xFFFF_EFFF_FFFF_0000);
    config.mappings.framebuffer = Mapping::FixedAddress(0xFFFF_A000_0000_0000);
    config
};

entry_point!(kmain, config = &CONFIG);

struct SingleThreadCS();
critical_section::set_impl!(SingleThreadCS);
/// SAFETY: While the OS kernel is running in a single thread, then disabling interrupts is a safe
/// to guarantee a critical section's conditions.
unsafe impl critical_section::Impl for SingleThreadCS {
    unsafe fn acquire() -> critical_section::RawRestoreState {
        let interrupts_enabled = arch::interrupts::are_enabled();
        arch::interrupts::disable();
        interrupts_enabled
    }

    unsafe fn release(interrupts_were_enabled: critical_section::RawRestoreState) {
        if interrupts_were_enabled {
            // SAFETY: Precondition.
            unsafe {
                arch::interrupts::enable();
            }
        }
    }
}
