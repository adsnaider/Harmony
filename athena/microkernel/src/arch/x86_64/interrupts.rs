use core::arch::asm;

use crate::arch::x86_64;
pub fn disable() {
    unsafe {
        asm!("cli", options(nostack, nomem));
    }
}

pub unsafe fn enable() {
    unsafe {
        asm!("sti", options(nostack, nomem));
    }
}

pub fn are_enabled() -> bool {
    let rflags = x86_64::registers::rflags();
    (rflags & (1 << 9)) > 0
}
