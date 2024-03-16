#![no_std]

use raw::syscall;

pub mod raw {
    use core::arch::asm;

    pub unsafe extern "sysv64" fn syscall(cap: usize, op: usize, a: usize, b: usize) -> isize {
        let out;
        asm!(
            "int 0x80",
            in("rdi") cap,
            in("rsi") op,
            in("rdx") a,
            in("rcx") b,
            lateout("rax") out,
        );
        out
    }
}

pub fn write(msg: &str) {
    let msg = msg.as_bytes();
    unsafe { syscall(usize::MAX, 0, msg.as_ptr() as usize, msg.len()) };
}
