//! A privileged (kernel) context.

use alloc::boxed::Box;

use x86_64::structures::paging::{Page, PageSize, Size4KiB};
use x86_64::VirtAddr;

use super::Regs;
use crate::arch::mm;
use crate::{dbg, sched};

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
        dbg!(regs.preserved.rsp);
        regs.rip = inner::<F> as u64;
        Self { regs, stack_page }
    }

    pub fn dummy() -> Self {
        KThread {
            regs: Regs::new(),
            stack_page: Page::from_start_address(VirtAddr::new(0)).unwrap(),
        }
    }

    /// Switches execution to the `self` context while saving the current state in `current`.
    ///
    /// On a follow up `switch` to `current`, the function will simply return back to this point.
    pub fn switch(&self, current: *mut Self) {
        self.regs
            .switch(unsafe { &mut (*current).regs as *mut Regs })
    }
}

// TODO: Drop.
