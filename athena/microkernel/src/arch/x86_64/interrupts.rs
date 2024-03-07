use core::arch::asm;

use crate::arch::x86_64;

/// Disable interrupts
pub fn disable() {
    // SAFETY: Disable interrupts can't lead to data races
    unsafe {
        asm!("cli", options(nostack, nomem));
    }
}

/// Enable interrupts
///
/// # Safety
///
/// Enabling interrupts introduces the possibility of data races which must be
/// accounted for
pub unsafe fn enable() {
    // SAFETY: Precondition
    unsafe {
        asm!("sti", options(nostack, nomem));
    }
}

/// Returns whether the interrupt flag is set
pub fn are_enabled() -> bool {
    let rflags = x86_64::registers::rflags();
    (rflags & (1 << 9)) > 0
}

#[cfg(test)]
mod tests {
    #[test_case]
    fn interrupts() {
        // SAFETY: No synchronization problems here, will restore in the end.
        unsafe {
            let enabled = super::are_enabled();
            super::enable();
            assert!(super::are_enabled());
            super::disable();
            assert!(!super::are_enabled());

            if enabled {
                super::enable();
            } else {
                super::disable();
            }
        }
    }
}
