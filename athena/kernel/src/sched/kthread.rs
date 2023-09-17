use alloc::boxed::Box;
use core::arch::asm;

use super::HasContext;
use crate::arch::context::Context;
use crate::arch::mm;
use crate::sched;

/// A kernel thread.
#[derive(Debug)]
pub struct KThread {
    context: Context,
}

impl KThread {
    /// Constructs a kernel thread context.
    pub fn new<F>(f: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        // WARNING: Fake "C" ABI where argument is passed on the stack!
        #[naked]
        unsafe extern "C" fn inner<F>(func: Box<F>) -> !
        where
            F: FnOnce() + Send + 'static,
        {
            // SAFETY: Argument is passed on the stack. `kstart` uses sysv64 abi which takes argument on `rdi`.
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
        let mut rsp = stack_page.start_address().as_u64() + stack_page.size();
        // SAFETY: Stack is big enough and `rsp` is correct.
        unsafe {
            Self::push(func as u64, &mut rsp);
            Self::push(inner::<F> as usize as u64, &mut rsp);
            Self::push(0, &mut rsp);
            Self::push(0, &mut rsp);
            Self::push(0, &mut rsp);
            Self::push(0, &mut rsp);
            Self::push(0, &mut rsp);
            Self::push(0, &mut rsp);
        }
        Self {
            context: Context::new(rsp, mm::active_page_table().start_address().as_u64()),
        }
    }

    unsafe fn push(val: u64, rsp: &mut u64) {
        // SAFETY: Precondition
        unsafe {
            *rsp -= 8;
            *(*rsp as *mut u64) = val;
        }
    }
}

impl HasContext for KThread {
    fn context(&self) -> *const crate::arch::context::Context {
        &self.context
    }

    fn context_mut(&mut self) -> *mut crate::arch::context::Context {
        &mut self.context
    }
}
