//! Interrupts initialization and handling.

use lazy_static::lazy_static;
use pc_keyboard::layouts::Us104Key;
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use pic8259::ChainedPics;
use spin::Mutex;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::{gdt, println, try_print};

const PIC1_OFFSET: u8 = 32;
const PIC2_OFFSET: u8 = PIC1_OFFSET + 8;

static PICS: Mutex<ChainedPics> = Mutex::new(
    // SAFETY: PIC Offsets don't collide with exceptions.
    unsafe { ChainedPics::new(PIC1_OFFSET, PIC2_OFFSET) },
);

const TIMER_INT: u8 = PIC1_OFFSET;
const KEYBOARD_INT: u8 = PIC1_OFFSET + 1;

/// Initializes the interrupt descriptor table.
fn init_idt() {
    lazy_static! {
        static ref IDT: InterruptDescriptorTable = {
            let mut idt = InterruptDescriptorTable::new();
            idt.breakpoint.set_handler_fn(breakpoint_handler);
            idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);
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
        };
    }
    IDT.load();
}

/// Initializes the IDT and sets up the 8259 PIC.
pub fn init() {
    init_idt();
    // SAFETY: PIC Initialization. We only initialize interrupts that we are currently handling.
    unsafe {
        let mut pics = PICS.lock();
        pics.initialize();
        pics.write_masks(0xFC, 0xFF);
    }
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let _ = try_print!(".");
    // SAFETY: Notify timer interrupt vector.
    unsafe {
        PICS.lock().notify_end_of_interrupt(TIMER_INT);
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<Us104Key, ScancodeSet1>> =
            Mutex::new(Keyboard::new(Us104Key, ScancodeSet1, HandleControl::Ignore));
    }

    let mut port = Port::new(0x60);
    // SAFETY: I/O read shouldn't have side effects.
    let scancode: u8 = unsafe { port.read() };

    let mut keyboard = KEYBOARD.lock();
    match keyboard.add_byte(scancode) {
        Ok(Some(event)) => {
            if let Some(key) = keyboard.process_keyevent(event) {
                match key {
                    DecodedKey::Unicode(character) => {
                        let _ = try_print!("{}", character);
                    }
                    DecodedKey::RawKey(key) => {
                        let _ = try_print!("{:?}", key);
                    }
                }
            }
        }
        Ok(None) => {}
        Err(e) => {
            let _ = try_print!("Keyboard error: {e:?}");
        }
    }

    // SAFETY: Notify keyboard interrupt vector.
    unsafe {
        PICS.lock().notify_end_of_interrupt(KEYBOARD_INT);
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

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT - {error_code}\n{stack_frame:#?}");
}
