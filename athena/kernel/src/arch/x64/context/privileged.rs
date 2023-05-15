//! A privileged (kernel) context.

use alloc::boxed::Box;

use x86_64::structures::paging::{Page, PageSize, Size4KiB};

use super::Regs;
use crate::arch::mm;
use crate::sched;

/// Kernel-based context.
#[derive(Debug)]
#[repr(C)]
pub struct KThread {
    regs: Regs,
    stack_page: Page<Size4KiB>,
}

impl KThread {
    /// Constructs a new context associated with the executor.
    pub fn new<F>(func: F) -> Self
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
            sched::kill();
        }
        let stack_page = mm::alloc_page().unwrap();
        let func = Box::into_raw(Box::new(func));
        let mut regs = Regs::new();
        // System-V ABI pushes int-like arguements to registers.
        regs.scratch.rdi = func as u64;
        regs.preserved.rsp = stack_page.start_address().as_u64() + Size4KiB::SIZE;
        regs.rip = inner::<F> as u64;
        Self { regs, stack_page }
    }
}

// TODO: Drop.
