#![no_std]
#![no_main]

use librs::kapi::raw_syscall;
use librs::println;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let _ = println!("{}", info);
    loop {}
}

#[no_mangle]
extern "C" fn _start() -> ! {
    let result = unsafe { raw_syscall(1, 2, 3, 4, 5, 6) };
    loop {}
}
