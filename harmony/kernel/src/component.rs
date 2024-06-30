//! A collection of resources provided to userspace threads.

use core::cell::RefCell;

use kapi::ops::cap_table::CapTableOp;
use kapi::ops::SyscallOp as _;
use kapi::raw::{CapError, CapId, SyscallArgs};
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
    pub fn exercise_cap(&self, capability: CapId, args: SyscallArgs) -> Result<usize, CapError> {
        let slot = RawCapEntry::get(self.resources.clone(), capability.into())
            .map_err(|_| CapError::Internal)?
            .ok_or(CapError::NotFound)?
            .get();
        match slot.resource {
            Resource::Empty => return Err(CapError::NotFound),
            Resource::CapEntry(capability_table) => {
                let operation =
                    CapTableOp::from_args(args).map_err(|_| CapError::InvalidArgument)?;
                match operation {
                    CapTableOp::Link {
                        other_table_cap,
                        slot,
                    } => {
                        let other_table =
                            RawCapEntry::get(self.resources.clone(), other_table_cap.into())
                                .map_err(|_| CapError::Internal)?
                                .ok_or(CapError::NotFound)?
                                .get();
                        let Resource::CapEntry(other_table) = other_table.resource else {
                            return Err(CapError::InvalidArgument);
                        };
                        let slot = RawCapEntry::index(capability_table, slot.into());
                        let mut current = slot.get();
                        current.child = Some(other_table);
                        slot.replace(current);
                        Ok(0)
                    }
                    CapTableOp::Unlink { slot } => {
                        let slot = RawCapEntry::index(capability_table, slot);
                        let mut current = slot.get();
                        current.child = None;
                        slot.replace(current);
                        Ok(0)
                    }
                    CapTableOp::Construct { kind: _ } => todo!(),
                    CapTableOp::Drop { slot: _ } => todo!(),
                    CapTableOp::Copy {
                        slot: _,
                        other_table_cap: _,
                        other_slot: _,
                    } => todo!(),
                }
            }
            Resource::Thread(_thread) => todo!(),
            Resource::PageTable { table: _, flags: _ } => todo!(),
        }
    }
}
