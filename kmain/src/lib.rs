#![no_std]

use core::panic::PanicInfo;

use bootinfo::Bootinfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn kmain(_bootinfo: &'static mut Bootinfo) -> ! {
    todo!();
}
