#![no_std]
#![no_main]
#![feature(naked_functions)]

mod serial;

use kapi::ops::cap_table::SlotId;
use kapi::raw::CapId;
use kapi::userspace::{CapTable, HardwareAccess, PageTable, PhysFrame, SyncCall, Thread};

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
    let mut stack = [0u8; 4096];
    unsafe {
        assert_eq!(
            sync_call
                .call(1, 2, 3, (&mut stack as *mut u8).add(stack.len()) as usize)
                .unwrap(),
            10
        );
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

    let mut stack = [0u8; 4096 * 2];
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

#[naked]
extern "C" fn sync_call(_a: usize, _b: usize, _c: usize, _d: usize) -> usize {
    extern "C" fn foo(a: usize, b: usize, c: usize) -> usize {
        let hardware_access = HardwareAccess::new(CapId::new(4));

        hardware_access.enable_ports().unwrap();
        sprintln!("Look ma! I'm a synchronous invocation");
        assert_eq!(a, 1);
        assert_eq!(b, 2);
        assert_eq!(c, 3);
        10
    }

    unsafe {
        core::arch::asm!(
            "mov rsp, rcx",
            "call {foo}",
            "mov rdi, 0",
            "mov rsi, 13",
            "mov rdx, rax",
            "call {raw_syscall}",
            foo = sym foo,
            raw_syscall = sym kapi::raw::raw_syscall,
            options(noreturn),
        )
    }
}
