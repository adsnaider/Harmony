//! A privileged (kernel) context.

use alloc::boxed::Box;
use core::arch::asm;

use x86_64::structures::paging::{Page, PageSize, Size4KiB};

use super::Context;
use crate::mm::alloc_page;

#[derive(Debug, Copy, Clone, Default)]
#[repr(packed)]
pub struct Regs {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rsp: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
}

impl Regs {
    /// Construct a new
    pub fn new() -> Self {
        Default::default()
    }
}

/// Kernel-based context.
#[derive(Debug)]
pub struct KernelContext {
    stack_page: Page<Size4KiB>,
    regs: Regs,
}

impl KernelContext {
    /// Constructs a new context associated with the executor.
    pub fn kthread<F>(func: F) -> Self
    where
        F: FnOnce() -> ! + Send + 'static,
    {
        extern "C" fn inner<F>(func: *mut F) -> !
        where
            F: FnOnce() -> ! + Send + 'static,
        {
            // SAFETY: We leaked it when we created the kthread.
            let func = unsafe { Box::from_raw(func) };
            func();
        }
        let stack_page = alloc_page().unwrap();
        let func = Box::into_raw(Box::new(func));
        let mut regs = Regs::new();
        regs.rdi = func as u64;
        regs.rsp = stack_page.start_address().as_u64() + Size4KiB::SIZE;
        regs.rip = inner::<F> as u64;
        Self { regs, stack_page }
    }
}

impl Context for KernelContext {
    fn switch(&self) -> ! {
        unsafe {
            asm!(
                "mov rbx, [rax + 8]",
                "mov rcx, [rax + 8*2]",
                "mov rdx, [rax + 8*3]",
                "mov rsi, [rax + 8*4]",
                "mov rdi, [rax + 8*5]",
                "mov rsp, [rax + 8*6]",
                "mov rbp, [rax + 8*7]",
                "mov r8, [rax + 8*8]",
                "mov r9, [rax + 8*9]",
                "mov r10, [rax + 8*10]",
                "mov r11, [rax + 8*11]",
                "mov r12, [rax + 8*12]",
                "mov r13, [rax + 8*13]",
                "mov r14, [rax + 8*14]",
                "mov r15, [rax + 8*15]",
                // "mov rflags, [rax + 8*17]",
                "push [rax + 8 * 16]",
                "mov rax, [rax]",
                "ret",
                in("rax") &self.regs,
                options(noreturn)
            )
        }
    }
}
