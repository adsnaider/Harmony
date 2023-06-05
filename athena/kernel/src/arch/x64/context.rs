//! A runnable context like a thread or process.

use alloc::boxed::Box;
use core::arch::asm;
use core::ptr::addr_of_mut;

use x86_64::structures::paging::{Page, PageSize, Size4KiB};

use crate::arch::mm;
use crate::sched;

/// Initializes the hardware capabilities for context switching.
pub fn init() {
    sce_enable();
}

/// A generic runnable context.
#[derive(Debug)]
#[repr(C)]
pub struct Context {
    stack_top: u64,
    l4_table: u64,
    variant: ContextVariant,
}

#[derive(Debug)]
#[repr(C)]
enum ContextVariant {
    /// Kernel thread.
    KThread(KThread),
    /// Main kernel thread.
    KMain,
}

/// A kernel thread context.
#[derive(Debug)]
struct KThread {
    _stack_page: Page<Size4KiB>,
}

impl Context {
    /// Performs the context switch into this context and stores the current state into `store`.
    ///
    /// # Safety
    ///
    /// `restore` and `store` pointers must be properly initialized contexts and `restore`.
    /// `restore` and `store` may not be equal.
    #[naked]
    pub unsafe extern "sysv64" fn switch(restore: *const Self, store: *mut Self) {
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

    unsafe fn push(val: u64, rsp: &mut u64) {
        unsafe {
            *rsp -= 8;
            *(*rsp as *mut u64) = val;
        }
    }

    /// Constructs a kernel thread context.
    pub fn kthread<F>(f: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        // WARNING: Fake "C" ABI where argument is passed on the stack!
        #[naked]
        extern "C" fn inner<F>(func: Box<F>) -> !
        where
            F: FnOnce() + Send + 'static,
        {
            unsafe {
                asm!("pop rdi", "call {ktstart}", "ud2", ktstart = sym ktstart::<F>, options(noreturn));
            }

            extern "sysv64" fn ktstart<F>(func: Box<F>) -> !
            where
                F: FnOnce() + Send + 'static,
            {
                // SAFETY: No locks are currently active in this context.
                unsafe { crate::arch::interrupts::enable() };
                // SAFETY: We leaked it when we created the kthread.
                {
                    func();
                }
                // Reenable interrupts if they got disabled.
                // SAFETY: No locks are currently active in this context.
                unsafe { crate::arch::interrupts::enable() };
                sched::exit();
            }
        }
        let stack_page = mm::alloc_page().unwrap();
        let func = Box::into_raw(Box::new(f));
        // System-V ABI pushes int-like arguements to registers.
        let mut rsp = stack_page.start_address().as_u64() + Size4KiB::SIZE;
        unsafe {
            Self::push(func as u64, &mut rsp);
            Self::push(inner::<F> as u64, &mut rsp);
            Self::push(0, &mut rsp);
            Self::push(0, &mut rsp);
            Self::push(0, &mut rsp);
            Self::push(0, &mut rsp);
            Self::push(0, &mut rsp);
            Self::push(0, &mut rsp);
        }
        Self {
            stack_top: rsp,
            l4_table: mm::active_page_table().start_address().as_u64(),
            variant: ContextVariant::KThread(KThread {
                _stack_page: stack_page,
            }),
        }
    }

    /// Creates the "main" contxt that is associated with the kernel entry point.
    ///
    /// Note that this function doesn't set the registers to anything meaningful, so it wouldn't be
    /// appropriate to jump directly into it.
    pub fn main() -> Self {
        Self {
            stack_top: 0,
            l4_table: mm::active_page_table().start_address().as_u64(),
            variant: ContextVariant::KMain,
        }
    }

    /// Performs a context switch that doesn't restore.
    ///
    /// # Safety
    ///
    /// * The `restore` pointer must be a valid `Context.
    pub unsafe fn jump(restore: *const Self) -> ! {
        let mut store = Context::main();
        // SAFETY: Preconditions
        unsafe {
            Self::switch(restore, addr_of_mut!(store));
        }
        unreachable!();
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
