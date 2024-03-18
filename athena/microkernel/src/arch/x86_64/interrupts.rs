use core::arch::asm;
use core::cell::RefCell;

use critical_section::Mutex;
use pic8259::ChainedPics;
use sync::cell::AtomicLazyCell;
use x86_64_impl::structures::idt::InterruptDescriptorTable;
use x86_64_impl::PrivilegeLevel;

use crate::arch::x86_64::{self, gdt};

mod handlers;

const PIC1_OFFSET: u8 = 32;
const PIC2_OFFSET: u8 = PIC1_OFFSET + 8;

static PICS: Mutex<RefCell<ChainedPics>> = Mutex::new(
    // SAFETY: PIC Offsets don't collide with exceptions.
    unsafe { RefCell::new(ChainedPics::new(PIC1_OFFSET, PIC2_OFFSET)) },
);

const TIMER_INT: u8 = PIC1_OFFSET;
const KEYBOARD_INT: u8 = PIC1_OFFSET + 1;

const SYSCALL_INT: u8 = 0x80;

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

/// Initializes the interrupt descriptor table.
fn init_idt() {
    static IDT: AtomicLazyCell<InterruptDescriptorTable> = AtomicLazyCell::new(|| {
        let mut idt = InterruptDescriptorTable::new();
        // Exceptions.
        idt.breakpoint.set_handler_fn(handlers::breakpoint);
        idt.general_protection_fault
            .set_handler_fn(handlers::general_protection_fault);
        idt.overflow.set_handler_fn(handlers::overflow);
        idt.divide_error.set_handler_fn(handlers::divide_error);
        idt.non_maskable_interrupt
            .set_handler_fn(handlers::non_maskable_interrupt);
        idt.bound_range_exceeded
            .set_handler_fn(handlers::bound_range_exceeded);
        idt.bound_range_exceeded
            .set_handler_fn(handlers::bound_range_exceeded);
        idt.debug.set_handler_fn(handlers::debug);
        idt.invalid_opcode.set_handler_fn(handlers::invalid_opcode);
        idt.device_not_available
            .set_handler_fn(handlers::device_not_available);
        idt.invalid_tss.set_handler_fn(handlers::invalid_tss);
        idt.segment_not_present
            .set_handler_fn(handlers::segment_not_present);
        idt.stack_segment_fault
            .set_handler_fn(handlers::stack_segment_fault);
        idt.x87_floating_point
            .set_handler_fn(handlers::x87_floating_point);
        idt.alignment_check
            .set_handler_fn(handlers::alignment_check);
        idt.machine_check.set_handler_fn(handlers::machine_check);
        idt.simd_floating_point
            .set_handler_fn(handlers::simd_floating_point);
        idt.virtualization.set_handler_fn(handlers::virtualization);
        idt.vmm_communication_exception
            .set_handler_fn(handlers::vmm_communication_exception);
        idt.security_exception
            .set_handler_fn(handlers::security_exception);
        // SAFETY: Stack index provided is valid and only used for the double fault handler.
        unsafe {
            idt.double_fault
                .set_handler_fn(handlers::double_fault)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
            idt.page_fault
                .set_handler_fn(handlers::page_fault)
                .set_stack_index(gdt::PAGE_FAULT_IST_INDEX);
        }
        // Syscall
        idt[SYSCALL_INT]
            .set_handler_fn(handlers::syscall_interrupt)
            .set_privilege_level(PrivilegeLevel::Ring3);

        // PIC interrupts
        idt[TIMER_INT].set_handler_fn(handlers::timer_interrupt);
        idt[KEYBOARD_INT].set_handler_fn(handlers::keyboard_interrupt);
        idt
    });
    IDT.load();
}

/// Initializes the IDT and sets up the 8259 PIC.
pub fn init() {
    critical_section::with(|cs| {
        init_idt();
        // SAFETY: PIC Initialization. We only initialize interrupts that we are currently handling.
        unsafe {
            let mut pics = PICS.borrow_ref_mut(cs);
            pics.initialize();
            pics.write_masks(0xFC, 0xFF);
        }
    })
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
