//! Capability-based system implementation

use core::ops::{Deref, DerefMut};

use kapi::{CapError, CapId};
use sync::cell::{AtomicRefCell, BorrowError};
use trie::{Ptr, Slot, TrieEntry, TrieIndexError};

use crate::arch::paging::{AnyPageTable, MemoryRegion};
use crate::component::ThreadControlBlock;
use crate::kptr::KPtr;
use crate::retyping::UntypedFrame;

#[derive(Default)]
pub struct CapSlot {
    child: Option<KPtr<RawCapEntry>>,
    capability: Capability,
}

pub struct InUse;

impl CapSlot {
    pub fn get_capability(&self) -> Capability {
        self.capability.clone()
    }

    pub fn set_child(&mut self, child: Option<CapabilityEntryPtr>) -> Option<KPtr<RawCapEntry>> {
        core::mem::replace(&mut self.child, child.map(|entry| entry.0))
    }

    pub fn set_capability(&mut self, new: Capability) -> Capability {
        core::mem::replace(&mut self.capability, new)
    }

    pub fn insert_capability(&mut self, new: Capability) -> Result<(), InUse> {
        if self.capability.is_empty() {
            self.capability = new;
            Ok(())
        } else {
            Err(InUse)
        }
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
}

#[repr(u8, C)]
#[derive(Default, Debug, Clone)]
pub enum Resource {
    #[default]
    Empty,
    CapEntry(CapabilityEntryPtr),
    Thread(KPtr<ThreadControlBlock>),
    PageTable {
        table: KPtr<AnyPageTable>,
        flags: PageCapFlags,
    },
    MemoryRegion(MemoryRegion),
}

impl Resource {
    pub const fn empty() -> Self {
        Self::Empty
    }

    pub const fn from_capability_table(table: CapabilityEntryPtr) -> Self {
        Self::CapEntry(table)
    }

    pub const fn from_tcb(tcb: KPtr<ThreadControlBlock>) -> Self {
        Self::Thread(tcb)
    }

    pub const fn from_page_table(table: KPtr<AnyPageTable>, level: u8) -> Self {
        Self::PageTable {
            table,
            flags: PageCapFlags::new(level),
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct Capability {
    pub resource: Resource,
}

impl Capability {
    pub const fn new(resource: Resource) -> Self {
        Self { resource }
    }
}

impl Capability {
    pub fn empty() -> Self {
        Self {
            resource: Resource::Empty,
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self.resource, Resource::Empty)
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PageCapFlags(u32);

impl PageCapFlags {
    const LEVEL_BITS: u32 = 0x0000_0003;
    pub const fn new(level: u8) -> Self {
        debug_assert!(level <= 4);
        Self((level as u32) & Self::LEVEL_BITS)
    }

    pub const fn level(&self) -> u8 {
        (self.0 & Self::LEVEL_BITS) as u8
    }
}
