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
            // SAFETY: Following ABI with iretq and we only wrap a C call with push/pop scratch registers.
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
    // SAFETY: No side effects from reading keyboard port.
    let _scancode: u8 = unsafe { port.read() };
    // print!("{}", scancode);
    print!("k");

    // SAFETY: Notify keyboard interrupt vector.
    unsafe {
        PICS.borrow_ref_mut(cs)
            .notify_end_of_interrupt(KEYBOARD_INT);
    }
});

// EXCEPTIONS

pub(super) extern "x86-interrupt" fn syscall_interrupt(stack_frame: InterruptStackFrame) {
    log::info!("SYSCALL REQUEST:\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn non_maskable_interrupt(stack_frame: InterruptStackFrame) {
    panic!("NON MASKABLE INTERRUPT :\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn bound_range_exceeded(stack_frame: InterruptStackFrame) {
    panic!("BOUND RANGE EXCEEDED:\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn debug(stack_frame: InterruptStackFrame) {
    panic!("DEBUG:\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn invalid_opcode(stack_frame: InterruptStackFrame) {
    panic!("INVALID OPCODE:\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn device_not_available(stack_frame: InterruptStackFrame) {
    panic!("DEVICE NOT AVAILABLE:\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn invalid_tss(stack_frame: InterruptStackFrame, code: u64) {
    panic!("INVALID TSS:\n{stack_frame:#?} ({code:X})");
}

pub(super) extern "x86-interrupt" fn segment_not_present(
    stack_frame: InterruptStackFrame,
    code: u64,
) {
    panic!("SEGMENT NOT PRESENT:\n{stack_frame:#?} ({code:X})");
}

pub(super) extern "x86-interrupt" fn stack_segment_fault(
    stack_frame: InterruptStackFrame,
    code: u64,
) {
    panic!("STACK SEGMENT FAULT:\n{stack_frame:#?} ({code:X})");
}

pub(super) extern "x86-interrupt" fn x87_floating_point(stack_frame: InterruptStackFrame) {
    panic!("X87 FLOATING POINT:\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn alignment_check(stack_frame: InterruptStackFrame, code: u64) {
    panic!("ALIGNMENT CHECK:\n{stack_frame:#?} ({code:X})");
}

pub(super) extern "x86-interrupt" fn machine_check(stack_frame: InterruptStackFrame) -> ! {
    panic!("MACHINE CHECK:\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn simd_floating_point(stack_frame: InterruptStackFrame) {
    panic!("SIMD FLOATING POINT:\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn virtualization(stack_frame: InterruptStackFrame) {
    panic!("VIRTUALIZATION:\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn vmm_communication_exception(
    stack_frame: InterruptStackFrame,
    code: u64,
) {
    panic!("VMM COMMUNICATION EXCEPTION:\n{stack_frame:#?} ({code:X})");
}

pub(super) extern "x86-interrupt" fn security_exception(
    stack_frame: InterruptStackFrame,
    code: u64,
) {
    panic!("SECURITY EXCEPTION:\n{stack_frame:#?} ({code:X})");
}

pub(super) extern "x86-interrupt" fn overflow(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION OVERFLOW:\n{stack_frame:#?}");
}
pub(super) extern "x86-interrupt" fn divide_error(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION DIVIDE ERROR:\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn general_protection_fault(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!("EXCEPTION: GENERAL PROTECTION - {error_code:#02X}\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn page_fault(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    panic!("EXCEPTION: PAGE FAULT - {error_code:#02X}\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn double_fault(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT - {error_code:#02X}\n{stack_frame:#?}");
}

pub(super) extern "x86-interrupt" fn breakpoint(stack_frame: InterruptStackFrame) {
    log::info!("EXCEPTION BREAKPOINT:\n{stack_frame:#?}");
}
