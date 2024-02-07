#![no_std]
#![no_main]

use librs::syscall;

#[no_mangle]
extern "C" fn _start() -> ! {
    unsafe {
        syscall(0, 1, 2, 3);
    }
    loop {}
}
