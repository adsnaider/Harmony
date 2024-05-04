//! A runnable and preemptable thread of execution.

use core::arch::asm;
use core::mem::MaybeUninit;

use super::paging::{AnyPageTable, RawFrame};
use crate::kptr::KPtr;

/// A generic runnable context.
///
/// The context provides two methods, [`Context::jump`] and [`Context::switch`]. These can be
/// used to switch the current thread of execution to a different context.
#[derive(Debug)]
#[repr(C)]
pub struct ExecutionContext {
    // TODO: Add all the registers here and get rid of per-process interrupt stack page
    rsp: u64,
    address_space: RawFrame,
}

impl ExecutionContext {
    pub fn addrspace(&self) -> KPtr<AnyPageTable> {
        unsafe { KPtr::from_frame_unchecked(self.address_space.into_kernel_unchecked()) }
    }

    pub unsafe fn uninit() -> Self {
        Self {
            rsp: 0,
            address_space: RawFrame::from_index(0),
        }
    }
    /// Creates a new context.
    pub unsafe fn new(rsp: u64, address_space: RawFrame) -> Self {
        Self { rsp, address_space }
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
        let mut store = MaybeUninit::uninit();
        // SAFETY: Preconditions
        unsafe {
            Self::switch(restore, store.as_mut_ptr());
        }
        unreachable!();
    }
}
