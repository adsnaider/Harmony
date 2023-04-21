//! x86-64 instructions.

/// Stops the CPU until the next interrupt.
pub fn hlt() {
    x86_64::instructions::hlt();
}
