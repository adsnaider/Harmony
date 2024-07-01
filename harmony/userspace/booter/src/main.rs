#![no_std]
#![no_main]

use librs::kapi::raw::raw_syscall;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use librs::println;
    let _ = println!("{}", info);
    loop {}
}

#[no_mangle]
extern "C" fn _start() -> ! {
    let _result = unsafe { raw_syscall(1, 2, 3, 4, 5, 6) };
    loop {}
}
