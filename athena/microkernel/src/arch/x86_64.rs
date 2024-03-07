use core::arch::asm;

pub mod interrupts {
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
        let rflags = x86_64::rflags();
        (rflags & (1 << 9)) > 0
    }
}

pub fn rflags() -> u64 {
    let rflags: u64;
    unsafe {
        asm!(
            "pushfq",
            "pop {rflags}",
            rflags = out(reg) rflags,
        )
    }
    rflags
}
