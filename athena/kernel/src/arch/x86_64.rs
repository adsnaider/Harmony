use core::arch::asm;

use crate::arch::timer::Pit8253;

pub mod instructions;
pub mod interrupts;
pub mod paging;
pub mod timer;

mod gdt;
mod registers;

pub fn init() {
    gdt::init();
    interrupts::init();
    let mut _timer = unsafe { Pit8253::steal().into_timer(5966) };
    log::info!("PIT Timer is initialized");
    sce_enable();

    log::info!("All x86-64 subsystems initialized");
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

fn sce_enable() {
    // SAFETY: Nothing special, just enabling Syscall extension.
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
    log::info!("Enabled SCE x86-64 extension");
}
