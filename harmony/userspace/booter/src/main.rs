#![no_std]
#![no_main]
#![feature(naked_functions)]

use core::cell::Cell;

use kapi::raw::CapId;
use kapi::userspace::cap_managment::{FrameAllocator, SelfCapabilityManager};
use kapi::userspace::structures::PhysFrame;
use kapi::userspace::Booter;
use serial::sprintln;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
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
extern "C" fn _start(lowest_frame: usize) -> ! {
    let resources = Booter::make();

    resources.hardware.enable_ports().unwrap();
    serial::init();

    let frames = FrameBumper::new(PhysFrame::new(lowest_frame));
    let _cap_manager =
        SelfCapabilityManager::new_with_start(resources.self_caps, CapId::new(6), &frames);
    sprintln!("Initializing user space");
    loop {}
}
