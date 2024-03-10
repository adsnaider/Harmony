use core::arch::asm;

use crate::arch::timer::Pit8253;

pub mod instructions;
pub mod interrupts;
pub mod paging;
pub mod port;
pub mod timer;

mod gdt;
mod registers;

pub fn init() {
    log::info!("Initializing GDT");
    gdt::init();
    log::info!("Initializing IDT");
    interrupts::init();
    log::info!("Initializing PIT timer");
    let mut _timer = unsafe { Pit8253::steal().into_timer(5966) };
}

/// Performs a `sysret` operation.
///
/// This will set the stack pointer to `rsp` and perform a jump to `rip`.
/// The processor will be switched to ring 3.
///
/// # Safety
///
/// The `rip` and `rsp` must be valid entrypoints for a user space process loaded
/// into the current address space.
#[naked]
pub unsafe extern "C" fn sysret(rip: u64, rsp: u64) -> ! {
    // SAFETY: This should be safe so long as rip and rsp are valid.
    unsafe {
        asm!(
            "mov ax, (4 * 8) | 3",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "push (4 * 8) | 3", // SS is handled by iret
            "push rsi",         // Stack pointer
            "push 0x202",       // rflags
            "push (3 * 8) | 3", // CS with RPL 3
            "push rdi",
            "iretq",
            options(noreturn)
        )
    }
}
