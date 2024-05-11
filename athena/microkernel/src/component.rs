use core::cell::UnsafeCell;

use elain::Align;
use kapi::{
    CapError, CapId, CapTableOp, MemoryRegionOp, PageTableOp, ResourceType, SyscallArgs, ThreadOp,
};
use x86_64_impl::structures::paging::PageTableFlags;

use crate::arch::execution_context::ExecutionContext;
use crate::arch::paging::page_table::PageTableOffset;
use crate::arch::paging::{AnyPageTable, RawFrame, PAGE_SIZE};
use crate::caps::{Capability, CapabilityEntryPtr, Resource};
use crate::kptr::KPtr;
use crate::retyping::TypedFrame;

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
                        let (resource_type, memory_cap, frame, slot, ..) = args.to_tuple();
                        let resource_type = ResourceType::try_from(resource_type as u8)
                            .map_err(|_| CapError::InvalidArgument)?;
                        let frame = RawFrame::try_from_start_address(frame as u64)
                            .map_err(|_| CapError::InvalidArgument)?;
                        let Resource::MemoryRegion(memory) =
                            self.caps.get(CapId::from(memory_cap as u32))?.resource
                        else {
                            return Err(CapError::InvalidArgument);
                        };
                        if !memory.includes_frame(&frame) {
                            return Err(CapError::FrameOutsideOfRegion);
                        }
                        let TypedFrame::Untyped(frame) = frame.as_typed() else {
                            return Err(CapError::InvalidArgument);
                        };
                        let resource: Resource = match resource_type {
                            ResourceType::CapabilityTable => {
                                Resource::from_capability_table(CapabilityEntryPtr::new(frame))
                            }
                            ResourceType::ThreadControlBlock => Resource::from_tcb(KPtr::new(
                                frame,
                                ThreadControlBlock::new(todo!(), todo!()),
                            )),
                            ResourceType::PageTable => Resource::from_page_table(
                                KPtr::new(frame, AnyPageTable::new()),
                                todo!(),
                            ),
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
                let level = flags.level();
                debug_assert!(level > 0 && level <= 4);
                match op {
                    PageTableOp::Link => {
                        let (entry, frame_cap, frame, ..) = args.to_tuple();
                        let entry =
                            PageTableOffset::new(entry).map_err(|_| CapError::InvalidArgument)?;
                        if level == 4 && !entry.is_lower_half() {
                            return Err(CapError::InvalidArgument);
                        }
                        let frame_cap = self.caps().get(CapId::from(frame_cap as u32))?;
                        if level == 1 {
                            let Resource::MemoryRegion(region) = frame_cap.resource else {
                                return Err(CapError::InvalidArgument);
                            };
                            let frame = RawFrame::try_from_start_address(frame as u64)
                                .map_err(|_| CapError::InvalidArgument)?;
                            if !region.includes_frame(&frame) {
                                return Err(CapError::FrameOutsideOfRegion);
                            }
                            let TypedFrame::User(frame) = frame.as_typed() else {
                                return Err(CapError::FrameNotUser);
                            };
                            let frame = frame.into_raw();
                            unsafe {
                                table.map(
                                    entry,
                                    frame,
                                    PageTableFlags::PRESENT
                                        | PageTableFlags::WRITABLE
                                        | PageTableFlags::USER_ACCESSIBLE,
                                );
                            }
                        } else {
                            let Resource::PageTable {
                                table: pointee,
                                flags,
                            } = frame_cap.resource
                            else {
                                return Err(CapError::InvalidArgument);
                            };
                            if flags.level() != level - 1 {
                                return Err(CapError::InvalidArgument);
                            }
                            let frame = pointee.into_raw();
                            unsafe {
                                table.map(
                                    entry,
                                    frame,
                                    PageTableFlags::PRESENT
                                        | PageTableFlags::WRITABLE
                                        | PageTableFlags::NO_EXECUTE,
                                );
                            }
                        }
                    }
                    PageTableOp::Unlink => {
                        let (entry, ..) = args.to_tuple();
                        let entry =
                            PageTableOffset::new(entry).map_err(|_| CapError::InvalidArgument)?;
                        if level == 4 && !entry.is_lower_half() {
                            return Err(CapError::InvalidArgument);
                        }
                        // SAFETY: This is a usersapce only entry.
                        unsafe { table.unmap(entry) };
                    }
                }
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
