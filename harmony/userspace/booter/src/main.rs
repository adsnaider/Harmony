#![no_std]
#![no_main]

use librs::kapi::ops::cap_table::{CapTableOp, ConsArgs, ConstructArgs, SlotId, ThreadConsArgs};
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
    loop {}
}

#[no_mangle]
extern "C" fn _start() -> ! {
    let operation = CapTableOp::Construct(ConsArgs {
        kind: ConstructArgs::Thread(ThreadConsArgs {
            entry: foo as usize,
            stack_pointer: 0,
            cap_table: 0.into(),
            page_table: 2.into(),
        }),
        region: 0x14000,
        slot: SlotId::<128>::try_from(4).unwrap(),
    });
    println!("{:?}", operation);
    unsafe { operation.syscall(0.into()) }.expect("Error on syscall");
    loop {}
}
