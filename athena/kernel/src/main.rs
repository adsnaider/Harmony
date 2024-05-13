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

use limine::request::{HhdmRequest, MemoryMapRequest};
use limine::BaseRevision;
use sync::cell::AtomicLazyCell;

use crate::arch::interrupts;

pub mod arch;
pub mod bump_allocator;
pub mod syscall;

#[cfg(test)]
mod testing;

mod serial;

pub static PMO: AtomicLazyCell<usize> = AtomicLazyCell::new(|| {
    #[used]
    static HHDM: HhdmRequest = HhdmRequest::new();

    let pmo = HHDM
        .get_response()
        .expect("Missing Higher-half direct mapping response from limine")
        .offset();
    // PMO must be on the higher half
    assert!(pmo > 0xFFFF_8000_0000_0000);
    pmo as usize
});

pub fn init() {
    #[used]
    static BASE_REVISION: BaseRevision = BaseRevision::with_revision(1);

    #[used]
    static mut MEMORY_MAP: MemoryMapRequest = MemoryMapRequest::new();
    interrupts::disable();

    serial::init();
    assert!(
        BASE_REVISION.is_supported(),
        "Limine revision not supported"
    );

    arch::init();

    log::info!("Got physical memory offset from limine at {:#X}", *PMO);

    let _memory_map = unsafe {
        MEMORY_MAP
            .get_response_mut()
            .expect("Missing memory map from Limine")
            .entries_mut()
    };
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
    init();
    loop {}
}
