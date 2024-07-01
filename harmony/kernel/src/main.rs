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

use limine::memory_map::Entry;
use limine::request::{HhdmRequest, MemoryMapRequest, StackSizeRequest};
use limine::BaseRevision;
use sync::cell::AtomicLazyCell;

use crate::arch::interrupts;
use crate::arch::paging::VirtAddr;
use crate::retyping::RetypeTable;

pub mod arch;
pub mod bump_allocator;
pub mod caps;
pub mod component;
pub mod core_local;
pub mod kptr;
pub mod retyping;
pub mod syscall;

#[cfg(test)]
mod testing;

mod serial;

pub type MemoryMap = &'static mut [&'static mut Entry];

pub const UNTYPED_MEMORY_OFFSET: usize = 0x0000_7000_0000_0000;

pub static PMO: AtomicLazyCell<VirtAddr> = AtomicLazyCell::new(|| {
    #[used]
    static HHDM: HhdmRequest = HhdmRequest::new();

    let pmo = HHDM
        .get_response()
        .expect("Missing Higher-half direct mapping response from limine")
        .offset();
    // PMO must be on the higher half
    assert!(pmo >= 0xFFFF_8000_0000_0000);
    VirtAddr::new(pmo as usize)
});

#[cfg(not(test))]
#[no_mangle]
extern "C" fn kmain() -> ! {
    use arch::bootup::Process;
    use arch::exec::{ExecCtx, NoopSaver};
    use arch::paging::RawFrame;
    use bump_allocator::BumpAllocator;
    use caps::RawCapEntry;
    use component::Thread;
    use kptr::KPtr;

    init();

    let booter: ExecCtx = {
        let proc = include_bytes_aligned::include_bytes_aligned!(16, "../../../.build/booter");
        log::info!("Loading user process");
        let process =
            Process::load(proc, 10, UNTYPED_MEMORY_OFFSET, RawFrame::memory_limit()).unwrap();
        process.into_exec()
    };
    let mut fallocator = BumpAllocator::new();
    let resources = {
        let frame = fallocator.alloc_untyped_frame().unwrap();
        KPtr::new(frame, RawCapEntry::default()).unwrap()
    };
    let thread = {
        let frame = fallocator.alloc_untyped_frame().unwrap();
        KPtr::new(frame, Thread::new_with_ctx(booter, resources)).unwrap()
    };

    log::info!("Jumping to boot component");
    Thread::dispatch(thread, NoopSaver::new());
}

pub fn init() {
    #[used]
    static BASE_REVISION: BaseRevision = BaseRevision::with_revision(1);

    #[used]
    static mut MEMORY_MAP: MemoryMapRequest = MemoryMapRequest::new();

    #[used]
    static STACK_SIZE: StackSizeRequest = StackSizeRequest::new().with_size(0x32000);
    interrupts::disable();

    serial::init();
    assert!(
        BASE_REVISION.is_supported(),
        "Limine revision not supported"
    );

    arch::init();

    STACK_SIZE.get_response().unwrap();

    log::info!(
        "Got physical memory offset from limine at {:#X}",
        PMO.as_usize()
    );

    let memory_map = unsafe {
        MEMORY_MAP
            .get_response_mut()
            .expect("Missing memory map from Limine")
            .entries_mut()
    };
    RetypeTable::new(memory_map).unwrap().init().unwrap();
    log::info!("Initialized the retype table");

    component::init();
    log::info!("Initialized component system");
}

#[cfg(all(target_os = "none", not(test)))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // TODO: Reboot
    log::error!("{}", info);
    loop {}
}
