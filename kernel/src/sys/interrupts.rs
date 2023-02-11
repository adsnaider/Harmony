//! Interrupts initialization and handling.

use core::cell::RefCell;

use critical_section::{CriticalSection, Mutex};
use once_cell::sync::{Lazy, OnceCell};
use pic8259::ChainedPics;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

pub mod async_interrupt;

use self::async_interrupt::{BoundedBufferInterrupt, InterruptCounterCore, InterruptWakerCore};
use super::gdt;

const PIC1_OFFSET: u8 = 32;
const PIC2_OFFSET: u8 = PIC1_OFFSET + 8;

static PICS: Mutex<RefCell<ChainedPics>> = Mutex::new(
    // SAFETY: PIC Offsets don't collide with exceptions.
    unsafe { RefCell::new(ChainedPics::new(PIC1_OFFSET, PIC2_OFFSET)) },
);

const TIMER_INT: u8 = PIC1_OFFSET;
const KEYBOARD_INT: u8 = PIC1_OFFSET + 1;

pub(super) static TIMER_INTERRUPT_CORE: InterruptCounterCore = InterruptCounterCore::new();
pub(super) static KEYBOARD_INTERRUPT_CORE: OnceCell<BoundedBufferInterrupt<u8>> = OnceCell::new();

/// Initializes the interrupt descriptor table.
fn init_idt() {
    static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.general_protection_fault
            .set_handler_fn(general_protection_fault_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        // SAFETY: Stack index provided is valid and only used for the double fault handler.
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }

        // PIC interrupts
        idt[TIMER_INT as usize].set_handler_fn(timer_interrupt_handler);
        idt[KEYBOARD_INT as usize].set_handler_fn(keyboard_interrupt_handler);
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

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // SAFETY: An interrupt cannot be interrupted. This is reasonable in single threaded code.
    let cs = unsafe { CriticalSection::new() };
    TIMER_INTERRUPT_CORE.update_and_wake((), cs);
    // SAFETY: Notify timer interrupt vector.
    unsafe {
        PICS.borrow_ref_mut(cs).notify_end_of_interrupt(TIMER_INT);
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // SAFETY: An interrupt cannot be interrupted. This is reasonable in single threaded code.
    let cs = unsafe { CriticalSection::new() };
    if let Some(core) = KEYBOARD_INTERRUPT_CORE.get() {
        let mut port = Port::new(0x60);
        // SAFETY: I/O read shouldn't have side effects.
        let scancode: u8 = unsafe { port.read() };
        core.update_and_wake(scancode, cs);
    }

    // SAFETY: Notify keyboard interrupt vector.
    unsafe {
        PICS.borrow_ref_mut(cs)
            .notify_end_of_interrupt(KEYBOARD_INT);
    }
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    try_println!("EXCEPTION BREAKPOINT:\n{stack_frame:#?}");
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!("EXCEPTION: GENERAL PROTECTION - {error_code}\n{stack_frame:#?}");
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    panic!("EXCEPTION: PAGE FAULT - {error_code:?}\n{stack_frame:#?}");
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT - {error_code}\n{stack_frame:#?}");
}