use core::ptr::NonNull;

use elain::Align;

use crate::arch::execution::ExecutionContext;
use crate::arch::PAGE_SIZE;
use crate::components::Component;

pub struct Thread {
    execution_context: ExecutionContext,
    resources: Component,
    _align: Align<PAGE_SIZE>,
}

impl Thread {
    pub fn new(context: ExecutionContext, resources: NonNull<Component>) -> Self {
        todo!();
    }

    pub fn activate(&self) {
        todo!();
    }
}
