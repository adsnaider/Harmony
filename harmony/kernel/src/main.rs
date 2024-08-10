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
    use arch::exec::NoopSaver;
    use arch::paging::RawFrame;
    use bump_allocator::BumpAllocator;
    use caps::RawCapEntry;
    use component::{Component, Thread};
    use kapi::ops::SlotId;
    use kptr::KPtr;
    use limine::request::ModuleRequest;
    use tar_no_std::TarArchiveRef;

    use crate::caps::{CapEntryExtension, PageCapFlags, Resource};

    init();
    let thread;

    {
        let (mut boot_regs, boot_page_table) = {
            static MODULES_REQUEST: ModuleRequest = ModuleRequest::new();

            let modules = MODULES_REQUEST.get_response().unwrap().modules();
            let initrd = modules
                .iter()
                .find(|module| module.path().ends_with(b"initrd.tar"))
                .expect("Bootloader didn't provide the initrd image");
            let initrd = unsafe {
                core::slice::from_raw_parts(initrd.addr(), initrd.size().try_into().unwrap())
            };

            let archive = TarArchiveRef::new(initrd).expect("Invalid initrd image");
            let proc = archive
                .entries()
                .find(|entry| {
                    entry.filename().as_str().expect("Invalid entry in initrd") == "booter"
                })
                .expect("Missing booter from initrd")
                .data();

            log::info!("Loading user process");
            let process = Process::load(
                proc,
                10,
                UNTYPED_MEMORY_OFFSET,
                RawFrame::memory_limit(),
                initrd,
            )
            .unwrap();
            process.into_parts()
        };
        let mut fallocator = BumpAllocator::new();
        let resources = {
            let frame = fallocator.alloc_untyped_frame().unwrap();
            KPtr::new(frame, RawCapEntry::default()).unwrap()
        };
        thread = {
            let frame = fallocator.alloc_untyped_frame().unwrap();
            boot_regs.scratch.rdi = fallocator.next_available().base().as_u64();
            KPtr::new(
                frame,
                Thread::new(
                    boot_regs,
                    Component::new(resources.clone(), boot_page_table),
                ),
            )
            .unwrap()
        };
        log::info!("Adding sync return capability to slot 0");
        resources
            .clone()
            .index_slot(SlotId::try_from(0).unwrap())
            .change(|cap| {
                cap.resource = Resource::SyncRet;
            });
        log::info!("Adding retype capability to slot 1");
        resources
            .clone()
            .index_slot(SlotId::try_from(1).unwrap())
            .change(|cap| {
                cap.resource = Resource::MemoryTyping;
            });
        log::info!("Adding self capability to slot 2");
        resources
            .clone()
            .index_slot(SlotId::try_from(2).unwrap())
            .change(|cap| {
                cap.resource = Resource::CapEntry(resources.clone());
            });
        log::info!("Adding thread capability to slot 3");
        resources
            .clone()
            .index_slot(SlotId::try_from(3).unwrap())
            .change(|cap| {
                cap.resource = Resource::Thread(thread.clone());
            });
        log::info!("Adding page table capability to slot 4");
        resources
            .clone()
            .index_slot(SlotId::try_from(4).unwrap())
            .change(|cap| {
                cap.resource = Resource::PageTable {
                    table: unsafe {
                        KPtr::from_frame_unchecked(
                            thread
                                .component()
                                .addrspace()
                                .l4_frame()
                                .try_as_kernel()
                                .unwrap(),
                        )
                    },
                    flags: PageCapFlags::new(4),
                };
            });

        log::info!("Adding hardware access capability to slot 5");
        resources
            .clone()
            .index_slot(SlotId::try_from(5).unwrap())
            .change(|cap| cap.resource = Resource::HardwareAccess);
    }

    log::info!("Jumping to boot component");
    Thread::dispatch(thread, NoopSaver::new());
}

pub fn init() {
    static BASE_REVISION: BaseRevision = BaseRevision::with_revision(1);
    static mut MEMORY_MAP: MemoryMapRequest = MemoryMapRequest::new();
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
