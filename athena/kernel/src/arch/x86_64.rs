use core::arch::asm;

use crate::arch::timer::Pit8253;

pub mod bootup;
pub mod exec;
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
