use critical_section::CriticalSection;
use x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode};

use super::{KEYBOARD_INT, PICS, TIMER_INT};

pub(super) extern "x86-interrupt" fn timer_interrupt(_stack_frame: InterruptStackFrame) {
    // SAFETY: An interrupt cannot be interrupted. This is reasonable in single threaded code.
    let cs = unsafe { CriticalSection::new() };

    // SAFETY: Notify timer interrupt vector.
    unsafe {
        PICS.borrow_ref_mut(cs).notify_end_of_interrupt(TIMER_INT);
    }
}

pub(super) extern "x86-interrupt" fn keyboard_interrupt(_stack_frame: InterruptStackFrame) {
    // SAFETY: An interrupt cannot be interrupted. This is reasonable in single threaded code.
    let cs = unsafe { CriticalSection::new() };

    // SAFETY: Notify keyboard interrupt vector.
    unsafe {
        PICS.borrow_ref_mut(cs)
            .notify_end_of_interrupt(KEYBOARD_INT);
    }
}

pub(super) extern "x86-interrupt" fn syscall_interrupt(stack_frame: InterruptStackFrame) {
    panic!("SYSCALL REQUEST:\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn breakpoint(stack_frame: InterruptStackFrame) {
    log::info!("EXCEPTION BREAKPOINT:\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn overflow(stack_frame: InterruptStackFrame) {
    log::info!("EXCEPTION OVERFLOW:\n{stack_frame:#?}");
}
pub(super) extern "x86-interrupt" fn divide_error(stack_frame: InterruptStackFrame) {
    log::info!("EXCEPTION DIVIDE ERROR:\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn general_protection_fault(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!("EXCEPTION: GENERAL PROTECTION - {error_code}\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn page_fault(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    panic!("EXCEPTION: PAGE FAULT - {error_code:?}\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn double_fault(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT - {error_code}\n{stack_frame:#?}");
}
