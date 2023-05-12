//! Kernel entry and executable. Ideally, this is just a thin wrapper over all of the kernel's
//! components.
#![no_std]
#![no_main]
#![feature(error_in_core)]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks)]

pub mod ksync;
pub mod sys;

// #[macro_use]
extern crate alloc;

use bootloader_api::config::Mapping;
use bootloader_api::{entry_point, BootInfo, BootloaderConfig};

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // Can't do much about errors at this point.
    let _ = try_println!("{}", info);
    loop {
        arch::inst::hlt();
    }
}

/// Kernel's starting point.
fn kmain(bootinfo: &'static mut BootInfo) -> ! {
    // SAFETY: The bootinfo is directly provided by the bootloader.
    unsafe { sys::init(bootinfo) }
    log::info!("Initialization sequence complete.");

    loop {}
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
        let interrupts_enabled = arch::int::are_enabled();
        arch::int::disable();
        interrupts_enabled
    }

    unsafe fn release(interrupts_were_enabled: critical_section::RawRestoreState) {
        if interrupts_were_enabled {
            arch::int::enable();
        }
    }
}
