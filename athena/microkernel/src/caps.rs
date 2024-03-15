//! Capability-based system implementation

use self::trie::CapabilityEntry;
use crate::arch::paging::PhysicalRegion;
use crate::kptr::KPtr;

mod trie;

#[repr(u8)]
#[derive(Debug, Clone)]
pub enum Resource {
    Empty,
    Untyped(PhysicalRegion),
    CapEntry(KPtr<CapabilityEntry>),
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Capability {
    resource: Resource,
    flags: CapFlags,
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
