/// Halts the CPU until the next wake up signal.
pub fn hlt() {
    // SAFETY: Halting the CPU is a safe operation
    unsafe {
        core::arch::asm!("hlt", options(nomem, preserves_flags, nostack));
    }
}

/// No-op instruction to do nothing for 1 cycle
pub fn nop() {
    // SAFETY: nop is a safe operation
    unsafe {
        core::arch::asm!("nop", options(nomem, preserves_flags, nostack));
    }
}
