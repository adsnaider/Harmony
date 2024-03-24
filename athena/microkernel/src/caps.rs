//! Capability-based system implementation

use kapi::{CapError, CapId, Operation, SyscallArgs};
use sync::cell::{AtomicRefCell, BorrowError};
use trie::{Ptr, Slot, TrieEntry};

use crate::arch::paging::PageTable;
use crate::component::ThreadControlBlock;
use crate::kptr::KPtr;
use crate::retyping::UntypedFrame;

#[derive(Default)]
struct CapSlot {
    child: Option<KPtr<RawCapEntry>>,
    capability: Capability,
}
#[derive(Default)]
pub struct AtomicCapSlot(AtomicRefCell<CapSlot>);

impl AtomicCapSlot {
    pub fn set_capability(&self, new: Capability) -> Result<Capability, BorrowError> {
        Ok(core::mem::replace(
            &mut self.0.borrow_mut()?.capability,
            new,
        ))
    }
}

const NUM_SLOTS: usize = 64;

impl Slot<NUM_SLOTS> for AtomicCapSlot {
    type Ptr = KPtr<RawCapEntry>;
    type Err = BorrowError;

    fn child(&self) -> Result<Option<Self::Ptr>, BorrowError> {
        Ok(self.0.borrow()?.child.clone())
    }
}

pub type RawCapEntry = TrieEntry<NUM_SLOTS, AtomicCapSlot>;

#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct CapabilityEntryPtr(KPtr<RawCapEntry>);

impl CapabilityEntryPtr {
    pub fn new(frame: UntypedFrame<'static>) -> Self {
        CapabilityEntryPtr(KPtr::new(frame, RawCapEntry::default()))
    }

    pub fn get(&self, cap: CapId) -> Result<Capability, CapError> {
        Ok(self.get_slot(cap)?.0.borrow()?.capability.clone())
    }

    pub fn get_slot(&self, cap: CapId) -> Result<impl Ptr<AtomicCapSlot>, CapError> {
        match RawCapEntry::get(self.0.clone(), cap.into())? {
            Some(slot) => Ok(slot),
            None => Err(CapError::NotFound),
        }
    }

    pub fn exercise(&self, cap: CapId, op: Operation, _args: SyscallArgs) -> Result<(), CapError> {
        let cap = self.get(cap)?;
        match cap.resource {
            Resource::Empty => return Err(CapError::NotFound),
            Resource::CapEntry(_cap_table) => match op {
                Operation::CapLink => todo!(),
                Operation::CapUnlink => todo!(),
                Operation::CapConstruct => todo!(),
                Operation::CapRemove => todo!(),
                _ => return Err(CapError::InvalidOpForResource),
            },
            Resource::Thread(thd) => match op {
                Operation::ThdActivate => ThreadControlBlock::activate(thd),
                _ => return Err(CapError::InvalidOpForResource),
            },
            Resource::PageTable(_) => match op {
                Operation::PageTableMap => todo!(),
                Operation::PageTableUnmap => todo!(),
                Operation::PageTableLink => todo!(),
                Operation::PageTableUnlink => todo!(),
                Operation::PageTableRetype => todo!(),
                _ => return Err(CapError::InvalidOpForResource),
            },
        }
        Ok(())
    }
}

#[repr(u8)]
#[derive(Default, Debug, Clone)]
pub enum Resource {
    #[default]
    Empty,
    CapEntry(KPtr<RawCapEntry>),
    Thread(KPtr<ThreadControlBlock>),
    PageTable(KPtr<PageTable>),
}

impl From<KPtr<RawCapEntry>> for Resource {
    fn from(value: KPtr<RawCapEntry>) -> Self {
        Self::CapEntry(value)
    }
}

impl From<CapabilityEntryPtr> for Resource {
    fn from(value: CapabilityEntryPtr) -> Self {
        Self::CapEntry(value.0)
    }
}

impl From<KPtr<ThreadControlBlock>> for Resource {
    fn from(value: KPtr<ThreadControlBlock>) -> Self {
        Self::Thread(value)
    }
}

impl From<KPtr<PageTable>> for Resource {
    fn from(value: KPtr<PageTable>) -> Self {
        Self::PageTable(value)
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct Capability {
    resource: Resource,
    flags: CapFlags,
}

impl Capability {
    pub fn new(resource: impl Into<Resource>, flags: CapFlags) -> Self {
        Self {
            resource: resource.into(),
            flags,
        }
    }
}

impl Capability {
    pub fn empty() -> Self {
        Self {
            resource: Resource::Empty,
            flags: CapFlags::empty(),
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CapFlags(u32);

impl Default for CapFlags {
    fn default() -> Self {
        Self::empty()
    }
}

impl CapFlags {
    pub fn empty() -> Self {
        Self(0)
    }
}
