//! Interrupts initialization and handling.

use core::cell::RefCell;

use critical_section::{CriticalSection, Mutex};
use once_cell::sync::Lazy;
use pc_keyboard::layouts::Us104Key;
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use pic8259::ChainedPics;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

use super::gdt;
use crate::println;

const PIC1_OFFSET: u8 = 32;
const PIC2_OFFSET: u8 = PIC1_OFFSET + 8;

static PICS: Mutex<RefCell<ChainedPics>> = Mutex::new(
    // SAFETY: PIC Offsets don't collide with exceptions.
    unsafe { RefCell::new(ChainedPics::new(PIC1_OFFSET, PIC2_OFFSET)) },
);

const TIMER_INT: u8 = PIC1_OFFSET;
const KEYBOARD_INT: u8 = PIC1_OFFSET + 1;

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
    crate::sys::time::tick();
    // SAFETY: Notify timer interrupt vector.
    unsafe {
        PICS.borrow_ref_mut(cs).notify_end_of_interrupt(TIMER_INT);
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // SAFETY: An interrupt cannot be interrupted. This is reasonable in single threaded code.
    let cs = unsafe { CriticalSection::new() };
    static KEYBOARD: Lazy<Mutex<RefCell<Keyboard<Us104Key, ScancodeSet1>>>> = Lazy::new(|| {
        Mutex::new(RefCell::new(Keyboard::new(
            Us104Key,
            ScancodeSet1,
            HandleControl::Ignore,
        )))
    });

    let mut port = Port::new(0x60);
    // SAFETY: I/O read shouldn't have side effects.
    let scancode: u8 = unsafe { port.read() };

    let mut keyboard = KEYBOARD.borrow_ref_mut(cs);
    match keyboard.add_byte(scancode) {
        Ok(Some(event)) => {
            if let Some(key) = keyboard.process_keyevent(event) {
                match key {
                    DecodedKey::Unicode(character) => {
                        let _ = print!("{}", character);
                    }
                    DecodedKey::RawKey(key) => {
                        let _ = print!("{:?}", key);
                    }
                }
            }
        }
        Ok(None) => {}
        Err(e) => {
            let _ = print!("Keyboard error: {e:?}");
        }
    }

    // SAFETY: Notify keyboard interrupt vector.
    unsafe {
        PICS.borrow_ref_mut(cs)
            .notify_end_of_interrupt(KEYBOARD_INT);
    }
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION BREAKPOINT:\n{stack_frame:#?}");
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
