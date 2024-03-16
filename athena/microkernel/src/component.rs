use core::cell::UnsafeCell;

use elain::Align;

use crate::arch::execution_context::ExecutionContext;
use crate::arch::paging::PAGE_SIZE;
use crate::caps::CapabilityEntry;
use crate::kptr::KPtr;

static mut CURRENT: Option<KPtr<ThreadControlBlock>> = None;

#[derive(Debug)]
pub struct ThreadControlBlock {
    caps: KPtr<CapabilityEntry>,
    execution_ctx: UnsafeCell<ExecutionContext>,
    _align: Align<PAGE_SIZE>,
}

impl ThreadControlBlock {
    pub fn new(caps: KPtr<CapabilityEntry>, execution_ctx: ExecutionContext) -> Self {
        Self {
            caps,
            execution_ctx: UnsafeCell::new(execution_ctx),
            _align: Align::default(),
        }
    }

    /// Activates this thread while deactivating the previously running one
    pub fn activate(this: KPtr<Self>) {
        // NOTE That this should be per core and the kernel should run without interrupts enabled
        unsafe {
            match CURRENT.replace(this) {
                Some(previous) => ExecutionContext::switch(
                    CURRENT.as_ref().unwrap_unchecked().execution_ctx.get(),
                    previous.execution_ctx.get(),
                ),
                None => {
                    ExecutionContext::jump(CURRENT.as_ref().unwrap_unchecked().execution_ctx.get())
                }
            }
        }
    }
}
