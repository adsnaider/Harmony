//! A runnable context like a thread or process.

use alloc::boxed::Box;
use core::arch::asm;
use core::ptr::{addr_of, addr_of_mut};

use x86_64::structures::paging::{Page, PageSize, PhysFrame, Size4KiB};

use crate::arch::mm;
use crate::{dbg, println, sched};

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

/*
impl Regs {
    /// Construct a new
    pub fn new() -> Self {
        Default::default()
    }

    /// Performs a context switch, to the `restore` state, saving the preserved registers in `store`.
    ///
    /// This has the effect of 1) switching execution context to the saved state in `restore` and 2)
    /// Saving the current state of execution to `store`, such that on a follow up switch, it will
    /// return back to the caller as if this function had been a no-op.
    #[naked]
    pub unsafe extern "sysv64" fn switch(restore: *const Self, store: *mut Self) {
        // SAFETY: Assuming the restore and store pointers are not aliasing and valid contexts,
        // this function will save the preserved registers (from sysv64 abi) alongside the stack
        // pointer and the return pointer (which both will be set appropriately for the return frame).
        // This works since the caller understands that scratch registers won't be saved.
        //
        // This the calls the `jump` function for the restore context which does the opposite,
        // restoring all the preserved registers before returning to the appropriate frame.
        unsafe {
            asm!(
                // Save current state
                // Return pointer
                "pop rax",
                "mov [rsi + 8*16], rax",
                "mov [rsi + 8*15], rsp",
                "mov [rsi], rbx",
                "mov [rsi + 8], rbp",
                "mov [rsi + 8*2], r12",
                "mov [rsi + 8*3], r13",
                "mov [rsi + 8*4], r14",
                "mov [rsi + 8*5], r15",
                "call {restore}",
                "ud2",
                restore = sym Self::jump,
                options(noreturn)
            )
        }
    }

    /// Performs a context switch, to the `restore` without saving the state.
    ///
    /// This restores the preserved registers only as the only safe way to save a context
    /// is through the `switch` function which guarantees that only the preserved registers
    /// must be saved.
    ///
    /// # Safety
    ///
    /// `restore` must be a valid context which was properly initialized and which state
    /// was saved with the `switch` function.
    #[naked]
    pub unsafe extern "sysv64" fn jump(restore: *const Self) -> ! {
        // SAFETY:
        unsafe {
            asm!(
                "pop rax", // Discard the return pointer.
                "mov rbx, [rdi]",
                "mov rbp, [rdi + 8*1]",
                "mov r12, [rdi + 8*2]",
                "mov r13, [rdi + 8*3]",
                "mov r14, [rdi + 8*4]",
                "mov r15, [rdi + 8*5]",
                "mov rsp, [rdi + 8*15]",
                "push [rdi + 8*16]", //rip
                "push [rdi + 8*17]", // rflags
                "popfq",
                "mov rdi, [rdi + 8*10]",
                "ret",
                options(noreturn)
            )
        }
    }
}
*/

impl Context {
    #[naked]
    pub extern "sysv64" fn switch(restore: *const Self, store: *mut Self) {
        unsafe {
            asm!(
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
        extern "cdecl" fn inner<F>(func: Box<F>) -> !
        where
            F: FnOnce() + Send + 'static,
        {
            // SAFETY: No locks are currently active in this context.
            // unsafe { crate::arch::interrupts::enable() };
            // SAFETY: We leaked it when we created the kthread.
            {
                func();
            }
            // Reenable interrupts if they got disabled.
            // SAFETY: No locks are currently active in this context.
            // unsafe { crate::arch::interrupts::enable() };
            sched::exit();
        }
        let stack_page = mm::alloc_page().unwrap();
        let func = Box::into_raw(Box::new(f));
        // System-V ABI pushes int-like arguements to registers.
        let mut rsp = stack_page.start_address().as_u64() + Size4KiB::SIZE;
        println!("rsp = {rsp:#X}");
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
        // regs.rflags = 0x2;
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
        Self::switch(restore, &mut store);
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
