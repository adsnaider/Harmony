//! A collection of resources provided to userspace threads.

use core::cell::RefCell;

use kapi::{CapError, CapId, SyscallArgs};
use sync::cell::AtomicOnceCell;

use crate::arch::exec::{ExecCtx, Regs};
use crate::arch::paging::page_table::AnyPageTable;
use crate::caps::{RawCapEntry, Resource};
use crate::core_local::CoreLocal;
use crate::kptr::KPtr;

static ACTIVE_THREAD: AtomicOnceCell<CoreLocal<RefCell<Option<KPtr<Thread>>>>> =
    AtomicOnceCell::new();

pub fn init() {
    let threads = CoreLocal::new_with(|_| RefCell::new(None));
    ACTIVE_THREAD.set(threads).unwrap();
}

// TODO: Implement thread migration
/// A user-space thread that provides a mechanism for dispatching.
///
/// Each thread has its own address space, execution context, and resource
/// table.
#[repr(align(4096))]
pub struct Thread {
    exec_ctx: ExecCtx,
    resources: KPtr<RawCapEntry>,
}

impl Thread {
    pub fn new(regs: Regs, l4_table: KPtr<AnyPageTable>, resources: KPtr<RawCapEntry>) -> Self {
        let exec_ctx = ExecCtx::new(l4_table.into_raw(), regs);
        Self {
            exec_ctx,
            resources,
        }
    }

    pub fn current() -> Option<KPtr<Thread>> {
        ACTIVE_THREAD.get().unwrap().get().borrow().clone()
    }

    pub fn dispatch(this: KPtr<Self>) -> ! {
        let mut current = ACTIVE_THREAD.get().unwrap().get().borrow_mut();
        current.replace(this.clone());
        this.exec_ctx.dispatch()
    }
}

impl Thread {
    pub fn exercise_cap(
        &self,
        capability: CapId,
        operation: usize,
        args: SyscallArgs,
    ) -> Result<usize, CapError> {
        let slot = RawCapEntry::get(self.resources.clone(), capability.into())
            .unwrap()
            .ok_or(CapError::NotFound)?
            .get();
        match slot.resource() {
            Resource::Empty => return Err(CapError::NotFound),
            Resource::CapEntry(capability_table) => todo!(),
            Resource::Thread(thread) => todo!(),
            Resource::PageTable { table, flags } => todo!(),
        }
    }
}
