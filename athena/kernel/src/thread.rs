use core::ptr::NonNull;

use crate::arch::execution::ExecutionContext;
use crate::components::Component;

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct Tid(u64);

pub struct Thread {
    execution_context: ExecutionContext,
    resources: Component,
}

impl Thread {
    pub fn new(context: ExecutionContext, resources: NonNull<Component>) -> Self {
        todo!();
    }

    pub fn activate(&self) {
        todo!();
    }
}
