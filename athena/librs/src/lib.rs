#![no_std]

use core::arch::asm;
use core::panic::PanicInfo;

pub unsafe fn syscall(cap: u64, a: u64, b: u64, c: u64) -> u64 {
    let mut out: u64;
    unsafe {
        asm!(
            "int 0x80",
            in("rdi") a,
            in("rsi") b,
            in("rdx") c,
            inlateout("rax") cap => out,
        )
    }
    out
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
