#![no_std]
#![no_main]
#![feature(abi_efiapi)]

use core::panic::PanicInfo;

use bootloader::sys;
use log;
use uefi::prelude::*;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if sys::is_init() {
        log::error!("{}", info);
    }
    loop {}
}

pub unsafe fn kernel_handoff(_entry: usize, _framebuffer: *const ()) -> ! {
    todo!();
}

#[entry]
fn efi_main(_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    // For the initial bootloader, it has to:
    // 1. Read the kernel program from disk.
    // 2. Get the framebuffer structure.
    // 3. Load the kernel to memory.
    // 4. Run the kernel passing in the framebuffer.
    sys::init(system_table);
    log::info!("Hello, UEFI!");
    let kernel = sys::fs::read("kernel").expect("Can't read kernel file.");
    log::info!("Got kernel.");

    loop {}
}
