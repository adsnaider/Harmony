//! A runnable context like a thread or process.

use alloc::boxed::Box;
use core::arch::asm;

use x86_64::structures::paging::{Page, PageSize, Size4KiB};

use crate::arch::mm;
use crate::sched;

// pub mod privileged;
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

/// A generic runnable context.
#[derive(Debug)]
#[repr(transparent)]
pub struct Context(ContextVariant);

#[derive(Debug)]
enum ContextVariant {
    /// Kernel thread.
    KThread(KThread),
}

/// A kernel thread context.
#[derive(Debug)]
struct KThread {
    regs: Regs,
    stack_page: Page<Size4KiB>,
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
    pub unsafe extern "sysv64" fn switch(restore: *const Self, store: *mut Self) {
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

    /// Performs a context switch, to the `self` without saving the state.
    #[naked]
    pub unsafe extern "sysv64" fn jump(restore: *const Self) -> ! {
        unsafe {
            asm!(
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

impl Context {
    /// Constructs a new kernel thread context.
    pub fn kthread<F>(f: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        Self(ContextVariant::kthread(f))
    }
    /// Performs a context switch.
    ///
    /// The `restore` context will be restored and the current context will be
    /// stored to `store`.
    ///
    /// # Safety
    ///
    /// * The `restore` and `store` pointers must be valid `Context`s.
    pub unsafe fn switch(restore: *const Self, store: *mut Self) {
        // SAFETY: repr(transparent)
        unsafe {
            ContextVariant::switch(
                restore as *const ContextVariant,
                store as *mut ContextVariant,
            )
        }
    }

    /// Performs a context switch that doesn't restore.
    ///
    /// # Safety
    ///
    /// * The `restore` pointer must be a valid `Context.
    pub unsafe fn jump(restore: *const Self) -> ! {
        // SAFETY: repr(transparent)
        unsafe {
            ContextVariant::jump(restore as *const ContextVariant);
        }
    }
}

impl ContextVariant {
    /// Constructs a new kernel thread context.
    pub fn kthread<F>(f: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        Self::KThread(KThread::new(f))
    }
    /// Performs a context switch.
    ///
    /// The `restore` context will be restored and the current context will be
    /// stored to `store`.
    ///
    /// # Safety
    ///
    /// * The `restore` and `store` pointers must be valid `Context`s.
    pub unsafe fn switch(restore: *const Self, store: *mut Self) {
        let (restore, store) = {
            let restore = unsafe { &*restore };
            let store = unsafe { &mut *store };

            let reg_restore = match restore {
                Self::KThread(kt) => &kt.regs,
            };
            let reg_store = match store {
                Self::KThread(kt) => &mut kt.regs,
            };
            (reg_restore, reg_store)
        };
        unsafe {
            Regs::switch(restore, store);
        }
    }

    /// Performs a context switch that doesn't restore.
    ///
    /// # Safety
    ///
    /// * The `restore` pointer must be a valid `Context.
    pub unsafe fn jump(restore: *const Self) -> ! {
        let restore = {
            let restore = unsafe { &*restore };

            let reg_restore = match restore {
                Self::KThread(kt) => &kt.regs,
            };

            reg_restore
        };
        unsafe {
            Regs::jump(restore);
        }
    }
}

impl KThread {
    /// Constructs the kernel thread context.
    pub fn new<F>(f: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        extern "sysv64" fn inner<F>(func: *mut F) -> !
        where
            F: FnOnce() + Send + 'static,
        {
            // SAFETY: We leaked it when we created the kthread.
            {
                let func = unsafe { Box::from_raw(func) };
                func();
            }
            sched::terminate();
        }
        let stack_page = mm::alloc_page().unwrap();
        let func = Box::into_raw(Box::new(f));
        let mut regs = Regs::new();
        // System-V ABI pushes int-like arguements to registers.
        regs.scratch.rdi = func as u64;
        regs.preserved.rsp = stack_page.start_address().as_u64() + Size4KiB::SIZE;
        regs.rip = inner::<F> as u64;
        Self { regs, stack_page }
    }
}

// TODO: Drop.

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
