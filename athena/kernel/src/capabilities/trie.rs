use elain::Align;
use tailcall::tailcall;

use super::caps::Capability;
use crate::arch::mm::kptr::KPtr;
use crate::arch::PAGE_SIZE;

const NODE_SIZE: usize = 64;
const NUM_NODES_PER_ENTRY: usize = PAGE_SIZE / NODE_SIZE;

#[derive(Debug, Clone)]
pub struct CapabilityEntry {
    nodes: [CapabilityNode; NUM_NODES_PER_ENTRY],
    _align: Align<PAGE_SIZE>,
}

// TODO: Padding of this is likely not ideal...
#[derive(Debug, Clone)]
struct CapabilityNode {
    capability: Capability,
    child: Option<KPtr<CapabilityEntry>>,
    _align: Align<NODE_SIZE>,
}

#[repr(transparent)]
pub struct CapId(usize);

impl CapabilityEntry {
    pub fn empty() -> Self {
        Self {
            nodes: core::array::from_fn(|_| CapabilityNode::empty()),
            _align: Default::default(),
        }
    }

    pub fn get(&self, id: CapId) -> Option<&Capability> {
        Self::get_inner(self, id.0)
    }

    #[tailcall]
    fn get_inner(this: &Self, id: usize) -> Option<&Capability> {
        let offset = id % NUM_NODES_PER_ENTRY;
        let id = id / NUM_NODES_PER_ENTRY;
        let node = &this.nodes[offset];
        if id == 0 {
            Some(&node.capability)
        } else {
            let child = this.nodes[offset].child.as_ref()?.as_ref();
            Self::get_inner(child, id)
        }
    }

    pub fn set(&self, offset: usize, capability: Capability) -> Option<Capability> {
        critical_section::with(|cs| {})
    }

    pub fn delete(&self, offset: usize) -> Option<Capability> {
        todo!();
    }

    pub fn link(&self, offset: usize, entry: KPtr<CapabilityEntry>) {
        todo!();
    }

    pub fn unlink(&self, offset: usize) -> Option<KPtr<CapabilityEntry>> {
        todo!();
    }
}

impl CapabilityNode {
    pub fn empty() -> Self {
        Self {
            capability: Capability::Empty,
            child: None,
            _align: Default::default(),
        }
    }
}

const _SIZE_AND_ALIGNMENT_REQUIRED: () = {
    assert!(core::mem::size_of::<CapabilityEntry>() == PAGE_SIZE);
    assert!(core::mem::align_of::<CapabilityEntry>() == PAGE_SIZE);
    assert!(core::mem::size_of::<CapabilityNode>() == NODE_SIZE);
    assert!(core::mem::align_of::<CapabilityNode>() == NODE_SIZE);
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
