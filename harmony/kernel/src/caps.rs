//! Capabilities to resources

use core::convert::Infallible;

use sync::cell::AtomicCell;
use trie::{Slot, TrieEntry};

use crate::arch::paging::page_table::AnyPageTable;
use crate::arch::paging::PAGE_SIZE;
use crate::component::Thread;
use crate::kptr::KPtr;

const SLOT_SIZE: usize = 32;
const NUM_SLOTS: usize = PAGE_SIZE / SLOT_SIZE;

/// A page-wide trie node for the capability tables.
pub type RawCapEntry = TrieEntry<NUM_SLOTS, AtomicCapSlot>;

#[derive(Debug, Default, Clone)]
pub struct CapSlot {
    child: Option<KPtr<RawCapEntry>>,
    resource: Resource,
}

pub struct InUse;

impl CapSlot {
    pub fn resource(&self) -> &Resource {
        &self.resource
    }

    pub fn replace_child(&mut self, child: Option<KPtr<RawCapEntry>>) -> Option<KPtr<RawCapEntry>> {
        core::mem::replace(&mut self.child, child)
    }

    pub fn set(&mut self, new: Resource) -> Resource {
        core::mem::replace(&mut self.resource, new)
    }

    pub fn insert(&mut self, new: Resource) -> Result<(), InUse> {
        if self.resource.is_empty() {
            self.resource = new;
            Ok(())
        } else {
            Err(InUse)
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Default)]
pub struct AtomicCapSlot(AtomicCell<CapSlot>);

impl AtomicCapSlot {
    pub fn replace(&self, slot: CapSlot) -> CapSlot {
        self.0.replace(slot)
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
pub enum Resource {
    #[default]
    Empty,
    CapEntry(KPtr<RawCapEntry>),
    Thread(KPtr<Thread>),
    PageTable {
        table: KPtr<AnyPageTable>,
        flags: PageCapFlags,
    },
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
