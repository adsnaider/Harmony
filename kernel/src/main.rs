//! Kernel entry and executable. Ideally, this is just a thin wrapper over all of the kernel's
//! components.
#![no_std]
#![no_main]
#![feature(allocator_api)]
#![feature(const_fn_floating_point_arithmetic)]
#![feature(negative_impls)]
#![feature(abi_x86_interrupt)]
#![feature(error_in_core)]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks)]

pub mod ksync;
pub mod proc;
pub(crate) mod singleton;
pub mod sys;

static INIT: &[u8] = include_bytes!("../programs/hello.bin");

#[macro_use]
extern crate alloc;

use bootloader_api::config::Mapping;
use bootloader_api::{entry_point, BootInfo, BootloaderConfig};

use crate::proc::Process;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // Can't do much about errors at this point.
    let _ = try_println!("{}", info);
    loop {
        x86_64::instructions::hlt();
    }
}

/// Kernel's starting point.
fn kmain(bootinfo: &'static mut BootInfo) -> ! {
    // SAFETY: The bootinfo is directly provided by the bootloader.
    let _tasks = unsafe { sys::init(bootinfo) };
    log::info!("Initialization sequence complete.");

    let process = Process::load(INIT, 1).unwrap();
    unsafe {
        process.exec();
    }
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
