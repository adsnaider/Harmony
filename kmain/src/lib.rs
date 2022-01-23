#![no_std]

use core::panic::PanicInfo;
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    todo!();
}

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    todo!();
}
