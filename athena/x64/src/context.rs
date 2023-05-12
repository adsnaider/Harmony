//! A runnable context like a thread or process.

use core::arch::asm;

pub mod userspace;

/// A context is a general abstraction to a thread of execution.
pub trait Context {
    /// Performs the context switch.
    unsafe fn switch(&self) -> !;
}

/// Initializes the hardware capabilities for context switching.
pub fn init() {
    sce_enable();
}

fn sce_enable() {
    unsafe {
        asm!(
            "mov rcx, 0xc0000082",
            "wrmsr",
            "mov rcx, 0xc0000080",
            "rdmsr",
            "or eax, 1",
            "wrmsr",
            "mov rcx, 0xc0000081",
            "rdmsr",
            "mov edx, 0x00180008",
            "wrmsr",
            out("rcx") _,
            out("eax") _,
            out("edx") _,
            options(nostack, nomem),
        );
    }
}
