#![no_std]
#![no_main]

use librs::raw::syscall;
use librs::write;

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    write("Panic invoked");
    loop {}
}

#[no_mangle]
extern "C" fn _start() -> ! {
    write("Hello world!");
    // let result = unsafe { syscall(1, 2, 3, 4) } as u8 % 10;
    write(core::str::from_utf8(b"01234").unwrap());
    loop {}
}
