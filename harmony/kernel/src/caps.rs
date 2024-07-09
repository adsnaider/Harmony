//! Capabilities to resources

use core::convert::Infallible;

use kapi::raw::{CapError, CapId};
use sync::cell::AtomicCell;
use trie::{Ptr, Slot, SlotId, TrieEntry};

use crate::arch::paging::page_table::AnyPageTable;
use crate::arch::paging::PAGE_SIZE;
use crate::component::{Component, Thread};
use crate::kptr::KPtr;

const SLOT_SIZE: usize = 64;
const NUM_SLOTS: usize = PAGE_SIZE / SLOT_SIZE;

/// A page-wide trie node for the capability tables.
pub type RawCapEntry = TrieEntry<NUM_SLOTS, AtomicCapSlot>;

pub struct WrongVariant;
impl TryFrom<Resource> for KPtr<RawCapEntry> {
    type Error = WrongVariant;

    fn try_from(value: Resource) -> Result<Self, Self::Error> {
        match value {
            Resource::CapEntry(entry) => Ok(entry),
            _ => Err(WrongVariant),
        }
    }
}
impl TryFrom<Resource> for (KPtr<AnyPageTable>, PageCapFlags) {
    type Error = WrongVariant;

    fn try_from(value: Resource) -> Result<Self, Self::Error> {
        match value {
            Resource::PageTable { table, flags } => Ok((table, flags)),
            _ => Err(WrongVariant),
        }
    }
}
impl TryFrom<Resource> for () {
    type Error = WrongVariant;

    fn try_from(value: Resource) -> Result<Self, Self::Error> {
        match value {
            Resource::Empty => Ok(()),
            _ => Err(WrongVariant),
        }
    }
}
impl TryFrom<Resource> for KPtr<Thread> {
    type Error = WrongVariant;

    fn try_from(value: Resource) -> Result<Self, Self::Error> {
        match value {
            Resource::Thread(thread) => Ok(thread),
            _ => Err(WrongVariant),
        }
    }
}

pub trait CapEntryExtension: Sized {
    fn find(self, cap: CapId) -> Result<impl Ptr<AtomicCapSlot>, CapError>;
    fn index_slot(self, slot: SlotId<NUM_SLOTS>) -> impl Ptr<AtomicCapSlot>;

    fn get_capability(self, cap: CapId) -> Result<CapSlot, CapError> {
        Ok(self.find(cap)?.get())
    }

    fn get_resource_as<T: TryFrom<Resource, Error = WrongVariant>>(
        self,
        cap: CapId,
    ) -> Result<T, CapError> {
        let cap = self.get_capability(cap)?;
        cap.resource
            .try_into()
            .map_err(|_| CapError::InvalidArgument)
    }
}

impl CapEntryExtension for KPtr<RawCapEntry> {
    fn find(self, cap: CapId) -> Result<impl Ptr<AtomicCapSlot>, CapError> {
        RawCapEntry::get(self, cap.into())
            .map_err(|_| CapError::Internal)?
            .ok_or(CapError::NotFound)
    }

    fn index_slot(self, slot: SlotId<NUM_SLOTS>) -> impl Ptr<AtomicCapSlot> {
        RawCapEntry::index(self, slot)
    }
}

#[derive(Debug, Default, Clone)]
pub struct CapSlot {
    pub child: Option<KPtr<RawCapEntry>>,
    pub resource: Resource,
}

pub struct InUse;

impl CapSlot {
    pub fn insert(&mut self, new: Resource) -> Result<(), InUse> {
        if self.resource.is_empty() {
            self.resource = new;
            Ok(())
        } else {
            Err(InUse)
        }
    }
}

#[repr(align(64))]
#[derive(Debug, Default)]
pub struct AtomicCapSlot(AtomicCell<CapSlot>);

impl AtomicCapSlot {
    pub fn replace(&self, slot: CapSlot) -> CapSlot {
        self.0.replace(slot)
    }

    pub fn change<F: FnOnce(&mut CapSlot)>(&self, fun: F) {
        let mut slot = self.get();
        fun(&mut slot);
        self.replace(slot);
    }

    pub fn get(&self) -> CapSlot {
        self.0.get_cloned()
    }
}

impl Slot<NUM_SLOTS> for AtomicCapSlot {
    type Ptr = KPtr<RawCapEntry>;
    type Err = Infallible;

    fn child(&self) -> Result<Option<Self::Ptr>, Self::Err> {
        Ok(self.0.get_cloned().child)
    }
}

const _SIZE_OF_ENTRY: () = {
    assert!(core::mem::size_of::<AtomicCapSlot>() == SLOT_SIZE);
    assert!(core::mem::size_of::<RawCapEntry>() == PAGE_SIZE);
    assert!(PAGE_SIZE % core::mem::align_of::<RawCapEntry>() == 0);
};

#[derive(Default, Debug, Clone)]
#[repr(align(8))]
pub enum Resource {
    #[default]
    Empty,
    CapEntry(KPtr<RawCapEntry>),
    Thread(KPtr<Thread>),
    PageTable {
        table: KPtr<AnyPageTable>,
        flags: PageCapFlags,
    },
    HardwareAccess,
    SyncCall {
        entry: usize,
        component: Component,
    },
    SyncRet,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct PageCapFlags(u8);

impl PageCapFlags {
    pub fn new(level: u8) -> Self {
        Self(level)
    }

    pub fn level(&self) -> u8 {
        self.0
    }
}

impl Resource {
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

impl<T> trie::Ptr<T> for KPtr<T> {}
