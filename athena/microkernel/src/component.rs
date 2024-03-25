use core::cell::UnsafeCell;

use elain::Align;
use kapi::{CapError, CapId, Operation, ResourceType, SyscallArgs};
use x86_64_impl::structures::paging::PageTableFlags;

use crate::arch::execution_context::ExecutionContext;
use crate::arch::paging::page_table::PageTableOffset;
use crate::arch::paging::{PageTable, PAGE_SIZE};
use crate::caps::{Capability, CapabilityEntryPtr, Resource};
use crate::kptr::KPtr;
use crate::USER_MAPPED_PHYS_OFFSET;

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

    pub fn addrspace(&self) -> KPtr<PageTable<4>> {
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
        op: Operation,
        args: SyscallArgs,
    ) -> Result<(), CapError> {
        let cap = self.caps.get(cap)?;
        match cap.resource {
            Resource::Empty => return Err(CapError::NotFound),
            Resource::CapEntry(cap_table) => match op {
                Operation::CapLink => {
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
                Operation::CapUnlink => {
                    let (slot, ..) = args.to_tuple();
                    cap_table.index(slot)?.borrow_mut()?.set_child(None);
                }
                #[allow(unreachable_code, unused_variables)]
                Operation::CapConstruct => {
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
                Operation::CapRemove => {
                    let (slot, ..) = args.to_tuple();
                    cap_table
                        .index(slot)?
                        .borrow_mut()?
                        .set_capability(Capability::empty());
                }
                _ => return Err(CapError::InvalidOpForResource),
            },
            Resource::Thread(thd) => match op {
                Operation::ThdActivate => ThreadControlBlock::activate(thd),
                _ => return Err(CapError::InvalidOpForResource),
            },
            Resource::PageTable { table, flags } => match flags.level() {
                0 => {
                    let table: KPtr<PageTable<0>> = unsafe { table.into_typed_table() };
                    match op {
                        Operation::PageTableMap => {
                            let (offset, frame_addr, attributes, ..) = args.to_tuple();
                            let offset = PageTableOffset::new(offset)
                                .map_err(|_| CapError::PageOffsetOutOfBounds)?;
                            let frame = self
                                .addrspace()
                                .translate_page((frame_addr + USER_MAPPED_PHYS_OFFSET) as u64)
                                .ok_or(CapError::UserMappedFramePermissionError)?
                                .into_user()
                                .map_err(|_| CapError::FrameNotUser)?;
                            table.map(
                                offset,
                                frame,
                                // FIXME: Custom parameter
                                PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE,
                            );
                            // FIXME: Flush and figure out how to flush the other cores too...
                        }
                        Operation::PageTableUnmap => {
                            let (offset, ..) = args.to_tuple();
                            let offset = PageTableOffset::new(offset)
                                .map_err(|_| CapError::PageOffsetOutOfBounds)?;
                            table.unmap(offset);
                            // FIXME: Flush and figure out how to flush the other cores too...
                        }
                        _ => return Err(CapError::InvalidOpForResource),
                    }
                }
                level @ (1 | 2 | 3) => match op {
                    Operation::PageTableLink => todo!(),
                    Operation::PageTableUnlink => todo!(),
                    _ => return Err(CapError::InvalidOpForResource),
                },
                4 => match op {
                    Operation::PageTableLink => todo!(),
                    Operation::PageTableUnlink => todo!(),
                    Operation::PageTableRetype => todo!(),
                    _ => return Err(CapError::InvalidOpForResource),
                },
                other => unreachable!("Unexpected page table level"),
            },
        }
        Ok(())
    }
}
