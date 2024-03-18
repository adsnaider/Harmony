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

pub mod arch;
pub mod caps;
pub mod component;
pub mod kptr;
pub mod retyping;
pub mod syscall;
pub(crate) mod util;

mod serial;
#[cfg(test)]
mod tests;

use limine::request::{HhdmRequest, MemoryMapRequest};
use limine::BaseRevision;
use sync::cell::AtomicLazyCell;

use crate::arch::interrupts;

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

fn init() {
    #[used]
    static BASE_REVISION: BaseRevision = BaseRevision::new();

    #[used]
    static mut MEMORY_MAP: MemoryMapRequest = MemoryMapRequest::new();

    interrupts::disable();

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

    retyping::init(memory_map);
}

#[cfg(not(test))]
#[no_mangle]
unsafe extern "C" fn kmain() -> ! {
    use arch::bootstrap::Process;
    use caps::CapabilityEntryPtr;
    use include_bytes_aligned::include_bytes_aligned;
    use util::FrameBumpAllocator;

    use crate::arch::execution_context::ExecutionContext;
    use crate::component::ThreadControlBlock;
    use crate::kptr::KPtr;

    init();

    let mut allocator = FrameBumpAllocator::new();

    log::info!("Loading boot process");
    let boot_process = {
        let proc = include_bytes_aligned!(16, "../../../.build/booter");
        Process::load(proc, &mut allocator).expect("Couldn't load the boot process")
    };
    log::info!("Allocating capability tables and TCB");
    let cap_table = CapabilityEntryPtr::new(allocator.alloc_frame().unwrap());
    let boot_thread = KPtr::new(
        allocator.alloc_frame().unwrap(),
        ThreadControlBlock::new(cap_table, ExecutionContext::uninit()),
    );
    ThreadControlBlock::set_as_current(boot_thread);
    log::info!("Executing boot process");

    boot_process.exec();
}
