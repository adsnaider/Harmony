//! A runnable context like a thread or process.

use core::arch::asm;

pub mod privileged;
// pub mod userspace;

/// Initializes the hardware capabilities for context switching.
pub fn init() {
    sce_enable();
}

#[derive(Debug, Copy, Clone, Default)]
#[allow(missing_docs, dead_code)]
#[repr(packed)]
struct PreservedRegisters {
    pub rbx: u64,
    pub rsp: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

#[derive(Debug, Copy, Clone, Default)]
#[allow(missing_docs, dead_code)]
#[repr(packed)]
struct ScratchRegisters {
    pub rax: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
}

#[derive(Debug, Copy, Clone, Default)]
#[repr(packed)]
#[allow(missing_docs, dead_code)]
struct Regs {
    pub preserved: PreservedRegisters,
    pub scratch: ScratchRegisters,

    pub rip: u64,
    pub rflags: u64,
}

impl Regs {
    /// Construct a new
    pub fn new() -> Self {
        Default::default()
    }

    /// Performs a context switch, to the `self` state, saving the preserved registers in `previous`.
    ///
    /// This has the effect of 1) switching execution context to the saved state in `self` and 2)
    /// Saving the current state of execution to `current`, such that on a follow up switch, it will
    /// return back to the caller as if this function had been a no-op.
    ///
    /// This function will also set interrupts back on.
    #[naked]
    pub extern "sysv64" fn switch(&self, current: *mut Regs) {
        unsafe {
            asm!(
                // Save current state

                // Return pointer
                "pop rax",
                "mov [rsi + 8*16], rax",
                "mov [rsi], rbx",
                "mov [rsi + 8], rsp",
                "mov [rsi + 8*2], rbp",
                "mov [rsi + 8*3], r12",
                "mov [rsi + 8*4], r13",
                "mov [rsi + 8*5], r14",
                "mov [rsi + 8*6], r15",
                // Restore the registers
                "mov rbx, [rdi]",
                "mov rsp, [rdi + 8]",
                "mov rbp, [rdi + 8*2]",
                "mov r12, [rdi + 8*3]",
                "mov r13, [rdi + 8*4]",
                "mov r14, [rdi + 8*5]",
                "mov r15, [rdi + 8*6]",
                "mov rax, [rdi + 8*7]",
                "mov rcx, [rdi + 8*8]",
                "mov rdx, [rdi + 8*9]",
                "mov rsi, [rdi + 8*10]",
                "mov r8, [rdi + 8*12]",
                "mov r9, [rdi + 8*13]",
                "mov r10, [rdi + 8*14]",
                "mov r11, [rdi + 8*15]",
                // rflags
                "push [rdi + 8*17]",
                "popfq",
                // Return pointer
                "push [rdi + 8*16]", //rip
                // Rdi
                "mov rdi, [rdi + 8*11]",
                "sti",
                "ret",
                options(noreturn)
            )
        }
    }
}

fn sce_enable() {
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
}
