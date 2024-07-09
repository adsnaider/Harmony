#![no_std]
#![no_main]

mod serial;

use librs::kapi::ops::cap_table::SlotId;
use librs::kapi::raw::CapId;
use librs::ops::{CapTable, HardwareAccess, PageTable, PhysFrame, SyncCall, Thread};

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    sprintln!("{}", info);
    loop {}
}

extern "C" fn foo(arg0: usize) -> ! {
    let hardware_access = HardwareAccess::new(CapId::new(4));
    hardware_access.enable_ports().unwrap();

    sprintln!("{:?}", arg0 as *const Thread);

    let main_thread = unsafe { &*(arg0 as *const Thread) };
    sprintln!("{:?}", main_thread);
    sprintln!("In a thread!");
    let sync_call = SyncCall::new(CapId::new(6));
    unsafe {
        sync_call.call(0, 1, 2, 3).unwrap();
        main_thread.activate().unwrap();
    }
    unreachable!();
}

#[no_mangle]
extern "C" fn _start(lowest_frame: usize) -> ! {
    let resources = CapTable::new(CapId::new(1));
    let current_thread = Thread::new(CapId::new(2));
    let page_table = PageTable::new(CapId::new(3));
    let hardware_access = HardwareAccess::new(CapId::new(4));

    hardware_access.enable_ports().unwrap();

    serial::init();

    sprintln!("{:?}", &current_thread as *const Thread);

    let mut stack = [0u8; 4096];
    unsafe {
        resources
            .make_thread(
                foo,
                (&mut stack as *mut u8).add(stack.len()),
                resources,
                page_table,
                SlotId::new(5).unwrap(),
                PhysFrame::new(lowest_frame),
                &current_thread as *const _ as usize,
            )
            .unwrap();
        resources
            .make_sync_call(sync_call, resources, page_table, SlotId::new(6).unwrap())
            .unwrap();
    };

    let t2 = Thread::new(CapId::new(5));
    unsafe {
        t2.activate().unwrap();
    }

    sprintln!("We are back!");
    loop {}
}

extern "C" fn sync_call(_a: usize, _b: usize, _c: usize, _d: usize) -> usize {
    todo!();
}
