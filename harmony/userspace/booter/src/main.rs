#![no_std]
#![no_main]

use librs::kapi::ops::cap_table::{CapTableOp, ConsArgs, ConstructArgs, SlotId, ThreadConsArgs};
use librs::kapi::ops::thread::ThreadOp;
use librs::kapi::ops::SyscallOp;
use librs::println;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use librs::println;
    let _ = println!("{}", info);
    loop {}
}

fn foo() -> ! {
    println!("In a thread!");
    unsafe {
        ThreadOp::Activate.syscall(1.into()).unwrap();
    }
    loop {}
}

#[no_mangle]
extern "C" fn _start(lowest_frame: usize) -> ! {
    println!("Lowest frame: {lowest_frame}");
    let mut stack = [0u8; 4096];
    let operation = CapTableOp::Construct(ConsArgs {
        kind: ConstructArgs::Thread(ThreadConsArgs {
            entry: foo as usize,
            stack_pointer: &mut stack as *mut u8 as usize,
            cap_table: 0.into(),
            page_table: 2.into(),
        }),
        region: lowest_frame,
        slot: SlotId::<128>::try_from(4).unwrap(),
    });
    println!("{:?}", operation);
    unsafe { operation.syscall(0.into()) }.expect("Error on syscall");
    let activate = ThreadOp::Activate;
    unsafe {
        activate.syscall(4.into()).unwrap();
    }
    println!("We are back!");
    loop {}
}
