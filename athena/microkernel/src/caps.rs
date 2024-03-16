//! Capability-based system implementation

use num_enum::TryFromPrimitive;

pub use self::trie::CapabilityEntry;
use crate::arch::paging::PhysicalRegion;
use crate::component::ThreadControlBlock;
use crate::kptr::KPtr;

mod trie;

#[repr(u8)]
#[derive(Debug, Clone)]
pub enum Resource {
    Empty,
    Untyped(PhysicalRegion),
    CapEntry(KPtr<CapabilityEntry>),
    Thread(KPtr<ThreadControlBlock>),
}

#[repr(C)]
#[derive(Debug, Clone)]
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
