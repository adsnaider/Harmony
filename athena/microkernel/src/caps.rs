//! Capability-based system implementation

use num_enum::TryFromPrimitive;
use sync::{AtomicRefCell, BorrowError};
use trie::{Slot, TrieEntry};

use crate::arch::paging::PhysicalRegion;
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

const NUM_SLOTS: usize = 64;

impl Slot<NUM_SLOTS> for AtomicCapSlot {
    type Ptr = KPtr<RawCapEntry>;
    type Err = BorrowError;

    fn child(&self) -> Result<Option<Self::Ptr>, BorrowError> {
        Ok(self.0.borrow()?.child.clone())
    }
}

impl From<BorrowError> for CapError {
    fn from(value: BorrowError) -> Self {
        match value {
            BorrowError::AlreadyBorrowed => CapError::BorrowError,
        }
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
    pub fn get(&self, cap: u32) -> Result<Capability, CapError> {
        match RawCapEntry::get(self.0.clone(), cap)? {
            Some(slot) => Ok(slot.0.borrow()?.capability.clone()),
            None => Err(CapError::NotFound),
        }
    }
}

#[repr(u8)]
#[derive(Default, Debug, Clone)]
pub enum Resource {
    #[default]
    Empty,
    Untyped(PhysicalRegion),
    CapEntry(KPtr<RawCapEntry>),
    Thread(KPtr<ThreadControlBlock>),
}

#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct Capability {
    resource: Resource,
    flags: CapFlags,
}

impl Capability {
    pub fn exercise(self, op: Operation) -> Result<(), CapError> {
        match self.resource {
            Resource::Empty => return Err(CapError::NotFound),
            Resource::Untyped(_) => todo!(),
            Resource::CapEntry(_) => todo!(),
            Resource::Thread(thd) => match op {
                Operation::ThdActivate => ThreadControlBlock::activate(thd),
            },
        }
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, TryFromPrimitive)]
#[repr(usize)]
pub enum Operation {
    ThdActivate = 0,
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

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum CapError {
    BorrowError = 1,
    NotFound = 2,
    InvalidOp = 3,
    InvalidOpForResource = 4,
}

impl From<<Operation as TryFrom<usize>>::Error> for CapError {
    fn from(_value: <Operation as TryFrom<usize>>::Error) -> Self {
        Self::InvalidOp
    }
}

impl CapError {
    pub fn to_errno(self) -> isize {
        let errno: isize = (self as u8).into();
        errno
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct CapId(usize);

impl From<usize> for CapId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<CapId> for usize {
    fn from(value: CapId) -> Self {
        value.0
    }
}
