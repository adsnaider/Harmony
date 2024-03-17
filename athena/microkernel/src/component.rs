use core::cell::UnsafeCell;

use elain::Align;

use crate::arch::execution_context::ExecutionContext;
use crate::arch::paging::PAGE_SIZE;
use crate::caps::{CapError, CapabilityEntryPtr, Operation};
use crate::kptr::KPtr;

static mut CURRENT: Option<KPtr<ThreadControlBlock>> = None;

#[derive(Debug)]
pub struct ThreadControlBlock {
    caps: CapabilityEntryPtr,
    execution_ctx: UnsafeCell<ExecutionContext>,
    _align: Align<PAGE_SIZE>,
}

impl ThreadControlBlock {
    pub fn new(caps: CapabilityEntryPtr, execution_ctx: ExecutionContext) -> Self {
        Self {
            caps,
            execution_ctx: UnsafeCell::new(execution_ctx),
            _align: Align::default(),
        }
    }

    pub fn exercise(&self, cap: usize, op: usize) -> Result<(), CapError> {
        let operation = Operation::try_from(op);
        log::debug!("Got operation: {operation:?}");

        let cap = self.caps.get(cap as u32)?;
        cap.exercise(Operation::try_from(op)?)
    }

    pub fn current() -> KPtr<Self> {
        unsafe { CURRENT.clone().unwrap() }
    }

    pub fn set_as_current(this: KPtr<Self>) {
        assert!(unsafe { CURRENT.replace(this) }.is_none());
    }

    /// Activates this thread while deactivating the previously running one
    pub fn activate(this: KPtr<Self>) {
        // NOTE That this should be per core and the kernel should run without interrupts enabled
        if Self::current() == this {
            return;
        }
        unsafe {
            let previous = CURRENT.replace(this).unwrap();
            ExecutionContext::switch(
                CURRENT.as_ref().unwrap_unchecked().execution_ctx.get(),
                previous.execution_ctx.get(),
            );
        }
    }
}
