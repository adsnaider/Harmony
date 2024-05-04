use core::cell::UnsafeCell;

use elain::Align;
use kapi::{
    CapError, CapId, CapTableOp, MemoryRegionOp, PageTableOp, ResourceType, SyscallArgs, ThreadOp,
};

use crate::arch::execution_context::ExecutionContext;
use crate::arch::paging::{AnyPageTable, PAGE_SIZE};
use crate::caps::{Capability, CapabilityEntryPtr, Resource};
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

    pub fn caps(&self) -> &CapabilityEntryPtr {
        &self.caps
    }

    pub fn addrspace(&self) -> KPtr<AnyPageTable> {
        unsafe { (*self.execution_ctx.get()).addrspace() }
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

    pub fn exercise_capability(
        &self,
        cap: CapId,
        op: usize,
        args: SyscallArgs,
    ) -> Result<(), CapError> {
        let cap = self.caps.get(cap)?;
        match cap.resource {
            Resource::Empty => return Err(CapError::NotFound),
            Resource::CapEntry(cap_table) => {
                let op = CapTableOp::try_from(op)?;
                match op {
                    CapTableOp::Link => {
                        let (other_table_cap, slot, ..) = args.to_tuple();
                        let other_table = self.caps.get(CapId::from(other_table_cap as u32))?;
                        let Resource::CapEntry(other_table) = other_table.resource else {
                            return Err(CapError::InvalidArgument);
                        };
                        cap_table
                            .index(slot)?
                            .borrow_mut()?
                            .set_child(Some(other_table));
                    }
                    CapTableOp::Unlink => {
                        let (slot, ..) = args.to_tuple();
                        cap_table.index(slot)?.borrow_mut()?.set_child(None);
                    }
                    CapTableOp::Construct => {
                        let (resource_type, _page, slot, ..) = args.to_tuple();
                        let resource_type = ResourceType::try_from(resource_type as u8)
                            .map_err(|_| CapError::InvalidArgument)?;
                        let resource: Resource = match resource_type {
                            ResourceType::CapabilityTable => todo!(),
                            ResourceType::ThreadControlBlock => todo!(),
                            ResourceType::PageTable => todo!(),
                        };
                        cap_table
                            .index(slot)?
                            .borrow_mut()?
                            .set_capability(Capability::new(resource));
                    }
                    CapTableOp::Drop => {
                        let (slot, ..) = args.to_tuple();
                        cap_table
                            .index(slot)?
                            .borrow_mut()?
                            .set_capability(Capability::empty());
                    }
                    CapTableOp::Copy => todo!(),
                }
            }
            Resource::Thread(thd) => {
                let op = ThreadOp::try_from(op)?;
                match op {
                    ThreadOp::Activate => ThreadControlBlock::activate(thd),
                    ThreadOp::ChangeAffinity => todo!(),
                }
            }
            Resource::PageTable { table, flags } => {
                let op = PageTableOp::try_from(op)?;
                todo!();
            }
            Resource::MemoryRegion(region) => {
                let op = MemoryRegionOp::try_from(op)?;
                match op {
                    MemoryRegionOp::Retype => todo!(),
                    MemoryRegionOp::Split => todo!(),
                }
            }
        }
        Ok(())
    }
}
