#![no_std]
#![no_main]
#![feature(naked_functions)]

use core::cell::Cell;
use core::mem::MaybeUninit;
use core::ops::Range;

use kapi::raw::CapId;
use kapi::userspace::cap_managment::{FrameAllocator, SelfCapabilityManager};
use kapi::userspace::structures::PhysFrame;
use kapi::userspace::Booter;
use loader::{Loader, MemFlags};
use tar_no_std::TarArchiveRef;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use serial::sprintln;
    sprintln!("{}", info);
    loop {}
}

struct FrameBumper(Cell<PhysFrame>);
impl FrameBumper {
    pub fn new(start: PhysFrame) -> Self {
        Self(Cell::new(start))
    }

    pub fn next(&self) -> PhysFrame {
        let frame = self.0.get();
        self.0.set(PhysFrame::new(frame.addr() + 0x1000));
        frame
    }
}

impl FrameAllocator for &'_ FrameBumper {
    fn alloc_frame(&mut self) -> PhysFrame {
        self.next()
    }
}

#[no_mangle]
extern "C" fn _start(lowest_frame: usize, initrd: *const u8, initrd_size: usize) -> ! {
    let resources = Booter::make();

    resources.hardware.enable_ports().unwrap();
    serial::init();
    let initrd = unsafe { core::slice::from_raw_parts(initrd, initrd_size) };
    log::info!(
        "Jumped to userspace: next_frame: {}, initrd: ({:?}, {})",
        lowest_frame,
        initrd.as_ptr(),
        initrd.len()
    );

    let frames = FrameBumper::new(PhysFrame::new(lowest_frame));
    let mut cap_manager =
        SelfCapabilityManager::new_with_start(resources.self_caps, CapId::new(6), &frames);

    let initrd = TarArchiveRef::new(initrd).expect("Bad initramfs");
    let _memory_manager = initrd
        .entries()
        .find(|entry| {
            entry.filename().as_str().expect("Invalid entry in initrd") == "memory_manager"
        })
        .expect("Missing memory_manager from initrd")
        .data();

    log::info!("Initializing user space");
    loop {}
}

pub struct ElfLoader {}

#[derive(Debug, Clone, Copy)]
pub enum LoadError {}

impl Loader for ElfLoader {
    type Error = LoadError;

    fn load_with<F>(
        &mut self,
        at: Range<usize>,
        source: F,
        rwx: MemFlags,
    ) -> Result<(), Self::Error>
    where
        F: Fn(usize) -> MaybeUninit<u8>,
    {
        todo!()
    }

    unsafe fn unload(&mut self, vrange: core::ops::Range<usize>) {
        todo!()
    }
}
