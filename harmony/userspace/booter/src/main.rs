#![no_std]
#![no_main]

use librs::kapi::ops::cap_table::SlotId;
use librs::kapi::raw::CapId;
use librs::ops::{CapTable, PageTable, PhysFrame, Thread};
use librs::println;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use librs::println;
    let _ = println!("{}", info);
    loop {}
}

extern "C" fn foo(arg0: usize) -> ! {
    println!("{:?}", arg0 as *const Thread);
    let main_thread = unsafe { &*(arg0 as *const Thread) };
    println!("{:?}", main_thread);
    println!("In a thread!");
    unsafe {
        main_thread.activate().unwrap();
    }
    unreachable!();
}

#[no_mangle]
extern "C" fn _start(lowest_frame: usize) -> ! {
    let resources = CapTable::new(CapId::new(0));
    let current_thread = Thread::new(CapId::new(1));
    let page_table = PageTable::new(CapId::new(2));

    println!("{:?}", &current_thread as *const Thread);

    let mut stack = [0u8; 4096];
    unsafe {
        resources
            .make_thread(
                foo,
                (&mut stack as *mut u8).add(stack.len()),
                resources,
                page_table,
                SlotId::new(4).unwrap(),
                PhysFrame::new(lowest_frame),
                &current_thread as *const _ as usize,
            )
            .unwrap()
    };
    let t2 = Thread::new(CapId::new(4));
    unsafe {
        t2.activate().unwrap();
    }

    println!("We are back!");
    loop {}
}
