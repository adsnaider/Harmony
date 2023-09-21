//! A runnable context like a thread or process.

use core::arch::asm;
use core::ptr::addr_of_mut;

use super::mm::paging::AddrSpace;

/// Initializes the hardware capabilities for context switching.
pub fn init() {
    sce_enable();
}

/// A generic runnable context.
///
/// The context provides two methods, [`Context::jump`] and [`Context::switch`]. These can be
/// used to switch the current thread of execution to a different context.
#[derive(Debug)]
#[repr(C)]
pub struct Context {
    stack_top: u64,
    address_space: AddrSpace,
}

impl Context {
    /// Creates an uninitialized context that cannot be jumped into.
    ///
    /// Note that this function doesn't set the registers to anything meaningful, so it wouldn't be
    /// appropriate to jump directly into it.
    pub fn uninit() -> Self {
        Self {
            stack_top: 0,
            address_space: AddrSpace::current(),
        }
    }

    /// Creates a new context.
    pub fn new(stack_top: u64, address_space: AddrSpace) -> Self {
        Self {
            stack_top,
            address_space,
        }
    }

    /// Performs the context switch into this context and stores the current state into `store`.
    ///
    /// # Safety
    ///
    /// `restore` and `store` pointers must be properly initialized contexts and `restore`.
    /// `restore` and `store` may not be equal.
    #[naked]
    pub unsafe extern "sysv64" fn switch(restore: *const Self, store: *mut Self) {
        // SAFETY: Assuming we did everything else right, this will save the caller saved registers and rsp before
        // jumping to a different context. When we come back, we restore the stack pointer and registers
        // and return back to the original return point.
        unsafe {
            asm!(
                // caller saved registers.
                "push rbx",
                "push rbp",
                "push r12",
                "push r13",
                "push r14",
                "push r15",
                "mov [rsi], rsp",     // Save stack top.
                "mov rsp, [rdi]",     // Restore old stack
                "mov rax, [rdi + 8]", // next task's cr3
                "mov rbx, cr3",       // current cr3
                "cmp rax, rbx",
                "je 2f",
                "mov cr3, rax",
                "2:",
                "pop r15",
                "pop r14",
                "pop r13",
                "pop r12",
                "pop rbp",
                "pop rbx",
                "ret",
                options(noreturn)
            )
        }
    }

    /// Performs a context switch that doesn't restore.
    ///
    /// # Safety
    ///
    /// * The `restore` pointer must be a valid `Context.
    pub unsafe fn jump(restore: *const Self) -> ! {
        let mut store = Context::uninit();
        // SAFETY: Preconditions
        unsafe {
            Self::switch(restore, addr_of_mut!(store));
        }
        unreachable!();
    }
}

fn sce_enable() {
    // SAFETY: This just enables system calls, no requirements necessary.
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
