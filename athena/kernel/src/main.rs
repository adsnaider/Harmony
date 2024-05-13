#![no_std]
#![no_main]
#![feature(naked_functions)]
#![cfg_attr(
    test,
    feature(custom_test_frameworks),
    test_runner(crate::testing::runner),
    reexport_test_harness_main = "test_main"
)]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]

#[cfg(test)]
mod testing;

mod serial;

pub fn init() {
    serial::init();
}

#[cfg(all(target_os = "none", not(test)))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // TODO: Reboot
    log::error!("{}", info);
    loop {}
}

#[cfg(not(test))]
#[no_mangle]
extern "C" fn kmain() -> ! {
    loop {}
}
