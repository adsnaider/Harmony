#![no_std]
#![no_main]
#![feature(abi_efiapi)]

use core::panic::PanicInfo;

use uefi::prelude::*;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    todo!();
}

#[entry]
fn efi_main(_handle: Handle, _system_table: SystemTable<Boot>) -> Status {
    todo!();
}
