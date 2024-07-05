use core::arch::asm;
use core::mem::MaybeUninit;

use x86_64_impl::registers::control::Cr2;
use x86_64_impl::structures::idt::{InterruptStackFrame, PageFaultErrorCode};

use super::{KEYBOARD_INT, PICS, TIMER_INT};
use crate::arch::exec::{ControlRegs, PreservedRegs, Regs, SaveState, ScratchRegs};
use crate::arch::x86_64::gdt;

pub struct SyscallCtx {
    pub control_regs: ControlRegs,
    pub preserved_regs: PreservedRegs,
}

impl SaveState for SyscallCtx {
    fn save_state(self, regs: &mut Regs) {
        regs.control = self.control_regs;
        regs.preserved = self.preserved_regs;
    }
}

impl SyscallCtx {
    /// Reads the syscall context from the stack
    ///
    /// # Safety
    ///
    /// Must be currently handling a syscall
    pub unsafe fn current() -> Self {
        let stack_end: *mut u64 = gdt::interrupt_stack_end().as_mut_ptr();
        let rsp = unsafe { *stack_end.sub(2) };
        let rflags = unsafe { *stack_end.sub(3) };
        let rip = unsafe { *stack_end.sub(5) };
        let mut preserved: MaybeUninit<PreservedRegs> = MaybeUninit::uninit();
        unsafe {
            core::ptr::copy_nonoverlapping(
                stack_end.sub(12) as *const PreservedRegs,
                preserved.as_mut_ptr(),
                1,
            );
        }
        Self {
            control_regs: ControlRegs { rflags, rsp, rip },
            preserved_regs: unsafe { preserved.assume_init() },
        }
    }
}

pub struct IrqCtx {
    pub control_regs: ControlRegs,
    pub preserved_regs: PreservedRegs,
    pub scratch_regs: ScratchRegs,
}

impl SaveState for IrqCtx {
    fn save_state(self, regs: &mut Regs) {
        regs.control = self.control_regs;
        regs.preserved = self.preserved_regs;
        regs.scratch = self.scratch_regs;
    }
}

impl IrqCtx {
    /// Reads the syscall context from the stack
    ///
    /// # Safety
    ///
    /// Must be currently handling a syscall
    pub unsafe fn current() -> Self {
        let stack_end: *mut u64 = gdt::interrupt_stack_end().as_mut_ptr();
        let rsp = unsafe { *stack_end.sub(2) };
        let rflags = unsafe { *stack_end.sub(3) };
        let rip = unsafe { *stack_end.sub(5) };
        let mut preserved: MaybeUninit<PreservedRegs> = MaybeUninit::uninit();
        unsafe {
            core::ptr::copy_nonoverlapping(
                stack_end.sub(12) as *const PreservedRegs,
                preserved.as_mut_ptr(),
                1,
            );
        }
        let mut scratch: MaybeUninit<ScratchRegs> = MaybeUninit::uninit();
        unsafe {
            core::ptr::copy_nonoverlapping(
                stack_end.sub(21) as *const ScratchRegs,
                scratch.as_mut_ptr(),
                1,
            );
        }
        Self {
            control_regs: ControlRegs { rflags, rsp, rip },
            preserved_regs: unsafe { preserved.assume_init() },
            scratch_regs: unsafe { scratch.assume_init() },
        }
    }
}

impl IrqCtx {}

macro_rules! push_scratch {
    () => {
        r#"
        push r11
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

macro_rules! push_preserved {
    () => {
        r#"
        push r15
        push r14
        push r13
        push r12
        push rbp
        push rbx
        "#
    };
}

macro_rules! pop_preserved {
    () => {
        r#"
        pop rbx
        pop rbp
        pop r12
        pop r13
        pop r14
        pop r15
        "#
    };
}

macro_rules! interrupt {
    ($name:ident, $handler:expr) => {
        #[naked]
        pub(super) extern "x86-interrupt" fn $name(_frame: InterruptStackFrame) {
            extern "C" fn inner() {
                #[allow(clippy::redundant_closure_call)]
                $handler();
            }
            // SAFETY: Following ABI with iretq and we only wrap a C call with push/pop scratch registers.
            unsafe {
                core::arch::asm!(
                    push_preserved!(),
                    push_scratch!(),
                    "call {inner}",
                    pop_scratch!(),
                    pop_preserved!(),
                    "iretq",
                    inner = sym inner,
                    options(noreturn),
                )
            }
        }
    }
}

interrupt!(timer_interrupt, || {
    // SAFETY: Notify timer interrupt vector.
    unsafe {
        PICS.notify_end_of_interrupt(TIMER_INT);
    }
});

interrupt!(keyboard_interrupt, || {
    // SAFETY: Notify keyboard interrupt vector.
    unsafe {
        PICS.notify_end_of_interrupt(KEYBOARD_INT);
    }
});

#[naked]
pub(super) extern "x86-interrupt" fn syscall_interrupt(stack_frame: InterruptStackFrame) {
    // SAFETY: Very thin wrapper over a syscall. We don't need to do callee saved since sysv64 abi will
    // take care of that.
    unsafe {
        asm!(
            push_preserved!(),
            "sub rsp, 8",
            "call {handle_syscall}",
            "add rsp, 8",
            pop_preserved!(),
            "iretq",
            handle_syscall = sym crate::syscall::handle,
            options(noreturn));
    }
}

// EXCEPTIONS

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
    let addrs: *const () = Cr2::read().unwrap().as_ptr();
    panic!("EXCEPTION: PAGE FAULT @ {addrs:#?} - {error_code:#02X}\n{stack_frame:#?}");
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

pub(super) extern "x86-interrupt" fn cp_protection_exception(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!("EXCEPTION CP PROTECTION: {error_code:#02X}\n{stack_frame:#?}");
}
pub(super) extern "x86-interrupt" fn hv_injection_exception(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION HV INJECTION:\n{stack_frame:#?}");
}
