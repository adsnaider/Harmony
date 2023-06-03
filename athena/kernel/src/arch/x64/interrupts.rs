//! Interrupt table and handlers.
use core::cell::RefCell;

use critical_section::{CriticalSection, Mutex};
use once_cell::sync::Lazy;
use pic8259::ChainedPics;
use x86_64::structures::idt::InterruptDescriptorTable;
use x86_64::PrivilegeLevel;

use super::gdt;

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

/// Enable interrupts.
pub unsafe fn enable() {
    x86_64::instructions::interrupts::enable();
}

/// Disable interrupts.
pub fn disable() {
    x86_64::instructions::interrupts::disable();
}

/// Returns true if interrupts are currently enabled.
pub fn are_enabled() -> bool {
    x86_64::instructions::interrupts::are_enabled()
}

/// Initializes the interrupt descriptor table.
fn init_idt() {
    static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(handlers::breakpoint);
        idt.general_protection_fault
            .set_handler_fn(handlers::general_protection_fault);
        idt.page_fault.set_handler_fn(handlers::page_fault);
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
        }
        // Syscall
        idt[SYSCALL_INT as usize]
            .set_handler_fn(handlers::syscall_interrupt)
            .set_privilege_level(PrivilegeLevel::Ring3);

        // PIC interrupts
        idt[TIMER_INT as usize].set_handler_fn(handlers::timer_interrupt);
        idt[KEYBOARD_INT as usize].set_handler_fn(handlers::keyboard_interrupt);
        idt
    });
    IDT.load();
}

/// Initializes the IDT and sets up the 8259 PIC.
pub fn init(cs: CriticalSection) {
    init_idt();
    // SAFETY: PIC Initialization. We only initialize interrupts that we are currently handling.
    unsafe {
        let mut pics = PICS.borrow_ref_mut(cs);
        pics.initialize();
        pics.write_masks(0xFC, 0xFF);
    }
}
