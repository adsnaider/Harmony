use elain::Align;
use tailcall::tailcall;

use crate::arch::paging::PAGE_SIZE;
use crate::caps::Capability;
use crate::kptr::KPtr;
use crate::sync::{AtomicRefCell, BorrowError};

const NUM_NODES_PER_ENTRY: usize = PAGE_SIZE / core::mem::size_of::<AtomicRefCell<Slot>>();

#[derive(Debug)]
pub struct CapabilityEntry {
    slots: [AtomicRefCell<Slot>; NUM_NODES_PER_ENTRY],
    _align: Align<PAGE_SIZE>,
}

#[derive(Debug)]
struct Slot {
    capability: Capability,
    child: Option<KPtr<CapabilityEntry>>,
}

#[repr(transparent)]
pub struct CapId(usize);

impl CapabilityEntry {
    pub fn empty() -> Self {
        Self {
            slots: core::array::from_fn(|_| AtomicRefCell::new(Slot::empty())),
            _align: Default::default(),
        }
    }

    pub fn get(&self, id: CapId) -> Result<Option<Capability>, BorrowError> {
        Self::get_inner(self, id.0)
    }

    #[tailcall]
    fn get_inner(this: &Self, id: usize) -> Result<Option<Capability>, BorrowError> {
        let offset = id % NUM_NODES_PER_ENTRY;
        let id = id / NUM_NODES_PER_ENTRY;
        let node = &this.slots[offset];
        if id == 0 {
            Ok(Some(node.borrow()?.capability.clone()))
        } else {
            let slot = this.slots[offset].borrow()?;
            let Some(child) = &slot.child else {
                return Ok(None);
            };
            Self::get_inner(child, id)
        }
    }

    pub fn set(&self, offset: usize, capability: Capability) -> Result<Capability, BorrowError> {
        let offset = offset % NUM_NODES_PER_ENTRY;
        let slot = &self.slots[offset];
        Ok(core::mem::replace(
            &mut slot.borrow_mut()?.capability,
            capability,
        ))
    }

    pub fn delete(&self, offset: usize) -> Result<Capability, BorrowError> {
        self.set(offset, Capability::empty())
    }

    pub fn link(
        &self,
        offset: usize,
        entry: KPtr<CapabilityEntry>,
    ) -> Result<Option<KPtr<CapabilityEntry>>, BorrowError> {
        self.set_link(offset, Some(entry))
    }

    pub fn unlink(&self, offset: usize) -> Result<Option<KPtr<CapabilityEntry>>, BorrowError> {
        self.set_link(offset, None)
    }

    fn set_link(
        &self,
        offset: usize,
        entry: Option<KPtr<CapabilityEntry>>,
    ) -> Result<Option<KPtr<CapabilityEntry>>, BorrowError> {
        let offset = offset % NUM_NODES_PER_ENTRY;
        let slot = &self.slots[offset];
        Ok(core::mem::replace(&mut slot.borrow_mut()?.child, entry))
    }
}

impl Slot {
    pub fn empty() -> Self {
        Self {
            capability: Capability::empty(),
            child: None,
        }
    }
}

const _SIZE_AND_ALIGNMENT_REQUIRED: () = {
    assert!(core::mem::size_of::<CapabilityEntry>() == PAGE_SIZE);
    assert!(core::mem::align_of::<CapabilityEntry>() == PAGE_SIZE);
};

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
