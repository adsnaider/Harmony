#![no_std]
#![no_main]

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    sprintln!("{}", info);
    loop {}
}

#[no_mangle]
extern "C" fn _start() -> ! {}
