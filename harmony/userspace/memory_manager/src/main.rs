#![no_std]
#![no_main]

use entry::entry;
use kapi::userspace::MemoryManager;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use serial::sprintln;

    sprintln!("{}", info);
    loop {}
}

#[entry]
fn main() -> ! {
    let MemoryManager {
        sync_ret,
        self_caps,
        self_paging,
        retype,
        hardware,
    } = MemoryManager::make();
    hardware.enable_ports().unwrap();
    serial::init();
    log::info!("Initializing memory manager");
    loop {}
}
