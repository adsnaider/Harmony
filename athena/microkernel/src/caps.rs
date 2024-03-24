//! Capability-based system implementation

use core::ops::{Deref, DerefMut};

use kapi::{CapError, CapId, Operation, ResourceType, SyscallArgs};
use sync::cell::{AtomicRefCell, BorrowError};
use trie::{Ptr, Slot, TrieEntry, TrieIndexError};

use crate::arch::paging::PageTable;
use crate::component::ThreadControlBlock;
use crate::kptr::KPtr;
use crate::retyping::UntypedFrame;

#[derive(Default)]
pub struct CapSlot {
    child: Option<KPtr<RawCapEntry>>,
    capability: Capability,
}

impl CapSlot {
    pub fn set_child(&mut self, child: Option<CapabilityEntryPtr>) -> Option<KPtr<RawCapEntry>> {
        core::mem::replace(&mut self.child, child.map(|entry| entry.0))
    }

    pub fn set_capability(&mut self, new: Capability) -> Capability {
        core::mem::replace(&mut self.capability, new)
    }
}

#[derive(Default)]
pub struct AtomicCapSlot(AtomicRefCell<CapSlot>);

impl AtomicCapSlot {
    pub fn borrow_mut(&self) -> Result<impl DerefMut<Target = CapSlot> + '_, BorrowError> {
        self.0.borrow_mut()
    }

    pub fn borrow(&self) -> Result<impl Deref<Target = CapSlot> + '_, BorrowError> {
        self.0.borrow()
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

    pub fn index(&self, offset: usize) -> Result<impl Ptr<AtomicCapSlot>, TrieIndexError> {
        RawCapEntry::index(self.0.clone(), offset)
    }

    pub fn exercise(&self, cap: CapId, op: Operation, args: SyscallArgs) -> Result<(), CapError> {
        let cap = self.get(cap)?;
        match cap.resource {
            Resource::Empty => return Err(CapError::NotFound),
            Resource::CapEntry(cap_table) => match op {
                Operation::CapLink => {
                    let (other_table_cap, slot, ..) = args.to_tuple();
                    let other_table = self.get(CapId::from(other_table_cap as u32))?;
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
                        .set_capability(Capability::new(resource, CapFlags::empty()));
                }
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
    CapEntry(CapabilityEntryPtr),
    Thread(KPtr<ThreadControlBlock>),
    PageTable(KPtr<PageTable>),
}

impl From<KPtr<RawCapEntry>> for Resource {
    fn from(value: KPtr<RawCapEntry>) -> Self {
        Self::CapEntry(CapabilityEntryPtr(value))
    }
}

impl From<CapabilityEntryPtr> for Resource {
    fn from(value: CapabilityEntryPtr) -> Self {
        Self::CapEntry(value)
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
