#![no_std]

use core::panic::PanicInfo;

pub mod caps {
    use core::arch::asm;

    pub unsafe fn syscall(cap: u64, op: u64) -> u64 {
        let mut out: u64;
        unsafe {
            asm!(
                "int 0x80",
                in("rdi") op,
                inlateout("rax") cap => out,
            )
        }
        out
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
