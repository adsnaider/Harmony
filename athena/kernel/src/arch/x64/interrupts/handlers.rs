use critical_section::CriticalSection;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode};

use super::{KEYBOARD_INT, PICS, TIMER_INT};

macro_rules! push_scratch {
    () => {
        r#"push r11
        push r10
        push r9
        push r8
        push rdi
        push rsi
        push rdx
        push rcx
        push rax
        "#
    };
}

macro_rules! pop_scratch {
    () => {
        r#"
        pop rax
        pop rcx
        pop rdx
        pop rsi
        pop rdi
        pop r8
        pop r9
        pop r10
        pop r11
        "#
    };
}

macro_rules! interrupt {
    ($name:ident, $handler:expr) => {
        #[naked]
        pub(super) extern "x86-interrupt" fn $name(_frame: InterruptStackFrame) {
            extern "C" fn inner() {
                $handler();
            }
            unsafe {
                core::arch::asm!(
                    push_scratch!(),
                    "call {inner}",
                    pop_scratch!(),
                    "iretq",
                    inner = sym inner,
                    options(noreturn),
                )
            }
        }
    }
}

interrupt!(timer_interrupt, || {
    use crate::sched;
    // SAFETY: An interrupt cannot be interrupted. This is reasonable in single threaded code.
    let cs = unsafe { CriticalSection::new() };

    // SAFETY: Notify timer interrupt vector.
    unsafe {
        PICS.borrow_ref_mut(cs).notify_end_of_interrupt(TIMER_INT);
    }
    sched::switch();
});

interrupt!(keyboard_interrupt, || {
    use crate::print;
    // SAFETY: An interrupt cannot be interrupted. This is reasonable in single threaded code.
    let cs = unsafe { CriticalSection::new() };

    let mut port = Port::new(0x60);
    let _scancode: u8 = unsafe { port.read() };
    // print!("{}", scancode);
    print!("k");

    // SAFETY: Notify keyboard interrupt vector.
    unsafe {
        PICS.borrow_ref_mut(cs)
            .notify_end_of_interrupt(KEYBOARD_INT);
    }
});

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
