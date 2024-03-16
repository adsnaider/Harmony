#![no_std]
#![no_main]

use librs::println;
use librs::raw::syscall;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let _ = println!("{}", info);
    loop {}
}

#[no_mangle]
extern "C" fn _start() -> ! {
    println!("Hello world");
    let result = unsafe { syscall(1, 2, 3, 4) };
    println!("Got {}", result);
    loop {}
}
