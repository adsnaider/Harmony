use core::ptr::{addr_of, addr_of_mut};
use core::sync::atomic::{AtomicPtr, Ordering};

use elain::Align;

use crate::arch::execution::ExecutionContext;
use crate::arch::mm::kptr::KPtr;
use crate::arch::PAGE_SIZE;
use crate::capabilities::trie::CapabilityEntry;

static ACTIVE: AtomicPtr<Thread> = AtomicPtr::new(core::ptr::null_mut());

#[repr(C)]
#[derive(Debug)]
pub struct Thread {
    execution_context: ExecutionContext,
    capabilities: KPtr<CapabilityEntry>,
    _align: Align<PAGE_SIZE>,
}

impl Thread {
    pub fn new(capabilities: KPtr<CapabilityEntry>, execution_context: ExecutionContext) -> Self {
        Self {
            capabilities,
            execution_context,
            _align: Align::default(),
        }
    }

    pub fn activate(this: &KPtr<Self>) {
        let new = this.as_ptr_mut();
        let old = ACTIVE.swap(new, Ordering::Release);

        unsafe {
            if old.is_null() {
                ExecutionContext::jump(addr_of!((*new).execution_context))
            } else {
                ExecutionContext::switch(
                    addr_of!((*new).execution_context),
                    addr_of_mut!((*old).execution_context),
                )
            }
        }
    }
}
