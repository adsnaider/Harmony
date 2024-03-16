#![no_std]
#![no_main]
#![cfg_attr(
    test,
    feature(custom_test_frameworks),
    test_runner(crate::tests::runner),
    reexport_test_harness_main = "test_main"
)]
#![feature(naked_functions)]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]

pub static PMO: Lazy<usize> = Lazy::new(|| {
    #[used]
    static HHDM: HhdmRequest = HhdmRequest::new();

    let pmo = HHDM
        .get_response()
        .expect("Missing Higher-half direct mapping response from limine")
        .offset();
    // PMO must be on the higher half
    assert!(pmo > 0x0000_8000_0000_0000);
    pmo as usize
});

pub mod arch;
pub mod caps;
pub mod component;
pub mod kptr;
pub mod retyping;
pub mod sync;
pub(crate) mod util;

mod serial;
#[cfg(test)]
mod tests;

use limine::memory_map::Entry;
use limine::request::{HhdmRequest, MemoryMapRequest};
use limine::BaseRevision;
use once_cell::sync::Lazy;

#[cfg(target_os = "none")]
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // FIXME: Reboot?
    log::error!("{}", info);
    loop {
        arch::instructions::hlt();
    }
}

fn init() -> &'static mut [&'static mut Entry] {
    #[used]
    static BASE_REVISION: BaseRevision = BaseRevision::new();

    #[used]
    static mut MEMORY_MAP: MemoryMapRequest = MemoryMapRequest::new();

    serial::init();
    log::info!("Serial logging initialized");

    assert!(BASE_REVISION.is_supported());
    arch::init();

    log::info!("Got physical memory offset from limine at {:#X}", *PMO);

    // TODO: VERIFY NULL PAGE EXISTS AT 0xFFFF_FFFF_7FFF_E000.
    let memory_map = unsafe {
        MEMORY_MAP
            .get_response_mut()
            .expect("Missing memory map from Limine")
            .entries_mut()
    };
    log::info!("Got memory map");
    log::info!("Initialization sequence complete");

    // TODO: Set up the retype tables

    memory_map
}

#[cfg(not(test))]
#[no_mangle]
unsafe extern "C" fn kmain() -> ! {
    use arch::bootstrap::Process;
    use include_bytes_aligned::include_bytes_aligned;
    use util::FrameBumpAllocator;

    let memory_map = init();
    let mut allocator = FrameBumpAllocator::new(memory_map);

    let boot_process = {
        let proc = include_bytes_aligned!(16, "../../userspace/init.bin");
        Process::load(proc, &mut allocator).expect("Couldn't load the boot process")
    };

    boot_process.exec();
}

struct SingleThreadCS();
critical_section::set_impl!(SingleThreadCS);
/// SAFETY: While the OS kernel is running in a single thread, then disabling interrupts is a safe
/// to guarantee a critical section's conditions.
unsafe impl critical_section::Impl for SingleThreadCS {
    unsafe fn acquire() -> critical_section::RawRestoreState {
        let interrupts_enabled = arch::interrupts::are_enabled();
        arch::interrupts::disable();
        interrupts_enabled
    }

    unsafe fn release(interrupts_were_enabled: critical_section::RawRestoreState) {
        if interrupts_were_enabled {
            // SAFETY: Precondition.
            unsafe {
                arch::interrupts::enable();
            }
        }
    }
}
