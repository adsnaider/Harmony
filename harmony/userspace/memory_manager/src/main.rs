#![no_std]
#![no_main]

use kapi::userspace::MemoryManager;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use serial::sprintln;

    sprintln!("{}", info);
    loop {}
}

#[no_mangle]
extern "C" fn _start() -> ! {
    let MemoryManager {
        sync_ret,
        self_caps,
        self_paging,
        retype,
        hardware,
    } = MemoryManager::make();
    hardware.enable_ports();
    log::info!("Initializing memory manager");
    loop {}
}
