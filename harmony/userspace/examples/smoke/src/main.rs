#![no_std]
#![no_main]
#![feature(naked_functions)]

use core::cell::Cell;

use kapi::ops::cap_table::{PageTableConsArgs, SyncCallConsArgs, ThreadConsArgs};
use kapi::ops::memory::RetypeKind;
use kapi::ops::paging::PermissionMask;
use kapi::raw::CapId;
use kapi::sync_call;
use kapi::userspace::cap_management::{FrameAllocator, SelfCapabilityManager};
use kapi::userspace::structures::{HardwareAccess, PhysFrame, Thread};
use kapi::userspace::Booter;
use serial::sprintln;
use stack_list::{StackList, StackNode};

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    sprintln!("{}", info);
    loop {}
}

extern "C" fn foo(arg0: usize) -> ! {
    let hardware_access = HardwareAccess::new(CapId::new(5));
    hardware_access.enable_ports().unwrap();

    sprintln!("{:?}", arg0 as *const Thread);

    let main_thread = unsafe { &*(arg0 as *const Thread) };
    sprintln!("{:?}", main_thread);
    sprintln!("In a thread!");
    unsafe {
        main_thread.activate().unwrap();
    }
    unreachable!();
}

static SYNC_STACKS: StackList<'static> = StackList::new();

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
    let mut cap_manager =
        SelfCapabilityManager::new_with_start(resources.self_caps, CapId::new(6), &frames);

    let p3 = cap_manager
        .allocate_capability()
        .make_page_table(PageTableConsArgs::new(frames.next(), 3))
        .unwrap();
    let p2 = cap_manager
        .allocate_capability()
        .make_page_table(PageTableConsArgs::new(frames.next(), 2))
        .unwrap();
    let p1 = cap_manager
        .allocate_capability()
        .make_page_table(PageTableConsArgs::new(frames.next(), 1))
        .unwrap();

    let thread_stack = frames.next();

    resources
        .retype
        .retype(thread_stack, RetypeKind::Retype2User)
        .unwrap();

    let sync_stack = frames.next();
    resources
        .retype
        .retype(sync_stack, RetypeKind::Retype2User)
        .unwrap();

    resources
        .self_paging
        .link(p3, 16, PermissionMask::WRITE)
        .unwrap();
    p3.link(p2, 0, PermissionMask::WRITE).unwrap();
    p2.link(p1, 0, PermissionMask::WRITE).unwrap();
    p1.map(0, thread_stack, PermissionMask::WRITE).unwrap();
    p1.map(2, sync_stack, PermissionMask::WRITE).unwrap();

    let tstack = 0x0000_0800_0000_0000 as *mut u8;
    resources.hardware.flush_page(tstack as usize).unwrap();
    let sstack_ptr = 0x0000_0800_0000_2000 as *mut u8;
    resources.hardware.flush_page(sstack_ptr as usize).unwrap();

    sprintln!("{:?}", &resources.self_thread);
    sprintln!("{:?}", &resources.self_thread as *const Thread);

    let sstack = unsafe { core::slice::from_raw_parts_mut(sstack_ptr, 0x1000) };
    let sstack = StackNode::new(sstack).unwrap();
    SYNC_STACKS.push_front(sstack);
    let t2;
    let scall;
    unsafe {
        t2 = cap_manager
            .allocate_capability()
            .make_thread(ThreadConsArgs::new(
                foo,
                tstack.add(0x1000),
                resources.self_caps,
                resources.self_paging,
                frames.next(),
                &resources.self_thread as *const _ as usize,
            ))
            .unwrap();
        scall = cap_manager
            .allocate_capability()
            .make_sync_call(SyncCallConsArgs::new(
                sync_call,
                resources.self_caps,
                resources.self_paging,
            ))
            .unwrap();
    }

    unsafe {
        t2.activate().unwrap();
    }

    sprintln!("We are back!");
    assert_eq!(scall.call(1, 2, 3, 4).unwrap(), 10);
    let stack = SYNC_STACKS.pop_front().unwrap().into_buffer();
    assert_eq!(stack.as_ptr(), sstack_ptr);
    assert_eq!(stack.len(), 4096);
    sprintln!("All done!");
    loop {}
}

sync_call!(sync_call, SYNC_STACKS, |a, b, c, d| {
    let hardware_access = HardwareAccess::new(CapId::new(5));

    hardware_access.enable_ports().unwrap();
    sprintln!("Look ma! I'm a synchronous invocation");
    assert_eq!(a, 1);
    assert_eq!(b, 2);
    assert_eq!(c, 3);
    assert_eq!(d, 4);
    10
});
