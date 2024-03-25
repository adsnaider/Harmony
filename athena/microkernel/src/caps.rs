//! Capability-based system implementation

use core::ops::{Deref, DerefMut};

use kapi::{CapError, CapId, Operation, ResourceType, SyscallArgs};
use sync::cell::{AtomicRefCell, BorrowError};
use trie::{Ptr, Slot, TrieEntry, TrieIndexError};

use crate::arch::paging::page_table::RawPageTable;
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
                        Operation::PageTableMap => todo!(),
                        Operation::PageTableUnmap => todo!(),
                        _ => return Err(CapError::InvalidOpForResource),
                    }
                }
                1 | 2 | 3 => match op {
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
                other => panic!("Unexpected page table level"),
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
    PageTable {
        table: KPtr<RawPageTable>,
        flags: PageCapFlags,
    },
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

    pub const fn from_page_table<const L: u8>(table: KPtr<PageTable<L>>) -> Self {
        Self::PageTable {
            table: table.into_raw_table(),
            flags: PageCapFlags::new(L),
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct Capability {
    resource: Resource,
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
