//! A collection of resources provided to userspace threads.

use core::cell::RefCell;

use kapi::ops::cap_table::{CapTableOp, ConstructArgs};
use kapi::ops::SyscallOp as _;
use kapi::raw::{CapError, CapId, SyscallArgs};
use sync::cell::AtomicOnceCell;

use crate::arch::exec::{ExecCtx, Regs};
use crate::arch::paging::page_table::{Addrspace, AnyPageTable, PageTableFlags};
use crate::arch::paging::{Page, RawFrame, VirtAddr};
use crate::caps::{CapEntryExtension as _, PageCapFlags, RawCapEntry, Resource};
use crate::core_local::CoreLocal;
use crate::kptr::KPtr;
use crate::UNTYPED_MEMORY_OFFSET;

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

    pub fn addrspace(&self) -> Addrspace<'_> {
        unsafe { Addrspace::from_frame(self.exec_ctx.l4_frame()) }
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
        let slot = self.resources.clone().find(capability)?.get();
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
                        let other_table: KPtr<RawCapEntry> =
                            self.resources.clone().get_resource_as(other_table_cap)?;
                        let slot = capability_table.index_slot(slot);
                        slot.change(|cap| {
                            cap.child = Some(other_table);
                        });
                        Ok(0)
                    }
                    CapTableOp::Unlink { slot } => {
                        let slot = capability_table.index_slot(slot);
                        slot.change(|cap| {
                            cap.child = None;
                        });
                        Ok(0)
                    }
                    CapTableOp::Construct { kind, region, slot } => {
                        if region > RawFrame::memory_limit() {
                            return Err(CapError::InvalidArgument);
                        }
                        let page_address = region + UNTYPED_MEMORY_OFFSET;
                        let region = Page::try_from_start_address(
                            VirtAddr::try_new(page_address)
                                .map_err(|_| CapError::InvalidArgument)?,
                        )
                        .map_err(|_| CapError::InvalidArgument)?;

                        let (frame, flags) = self
                            .addrspace()
                            .get(region)
                            .ok_or(CapError::InvalidArgument)?;
                        if !flags.contains(PageTableFlags::PRESENT) {
                            return Err(CapError::InvalidArgument);
                        }
                        let resource = match kind {
                            ConstructArgs::CapTable => {
                                let ptr = KPtr::new(frame, RawCapEntry::default())
                                    .map_err(|_| CapError::InvalidArgument)?;
                                Resource::CapEntry(ptr.into())
                            }
                            ConstructArgs::Thread {
                                entry,
                                stack_pointer,
                                cap_table,
                                page_table,
                            } => {
                                let regs = Regs {
                                    rip: entry as u64,
                                    rsp: stack_pointer as u64,
                                    rflags: 0x202,
                                    ..Default::default()
                                };
                                let cap_table: KPtr<RawCapEntry> =
                                    self.resources.clone().get_resource_as(cap_table)?;
                                let (page_table, flags): (KPtr<AnyPageTable>, PageCapFlags) =
                                    self.resources.clone().get_resource_as(page_table)?;
                                if !flags.level() == 4 {
                                    return Err(CapError::InvalidArgument);
                                }
                                Resource::Thread(
                                    KPtr::new(frame, Thread::new(regs, page_table, cap_table))
                                        .map_err(|_| CapError::InvalidArgument)?,
                                )
                            }
                            ConstructArgs::PageTable { level } => {
                                if level > 4 || level == 0 {
                                    return Err(CapError::InvalidArgument);
                                }
                                let table = if level == 4 {
                                    AnyPageTable::clone_kernel()
                                } else {
                                    AnyPageTable::new()
                                };
                                let flags = PageCapFlags::new(level);
                                Resource::PageTable {
                                    table: KPtr::new(frame, table)
                                        .map_err(|_| CapError::InvalidArgument)?,
                                    flags,
                                }
                            }
                        };
                        capability_table.index_slot(slot).change(|cap| {
                            cap.resource = resource;
                        });
                        Ok(0)
                    }
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
