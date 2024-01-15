use core::ptr::NonNull;

use tailcall::tailcall;

const NUM_NODES_PER_ENTRY: usize = 4096 / 64;

#[repr(align(4096))]
struct CapabilityEntry {
    nodes: [CapabilityNode; NUM_NODES_PER_ENTRY],
}

// TODO: Padding of this is likely not ideal...
#[repr(align(64))]
#[derive(Debug, Copy, Clone)]
struct CapabilityNode {
    capability: Capability,
    child: Option<NonNull<CapabilityEntry>>,
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum Capability {
    Empty = 0,
    Thread,
    CapTable,
    PageTable,
    MemoryTyping,
}

pub struct CapabilityTable {
    root: Option<NonNull<CapabilityEntry>>,
}

#[repr(transparent)]
pub struct CapId(usize);

impl CapabilityTable {
    pub fn new() -> Self {
        Self { root: None }
    }

    pub fn get(&self, id: CapId) -> Option<&Capability> {
        CapabilityEntry::get(unsafe { self.root?.as_ref() }, id.0)
    }
}

impl CapabilityEntry {
    pub fn empty() -> Self {
        Self {
            nodes: [CapabilityNode::empty(); NUM_NODES_PER_ENTRY],
        }
    }

    #[tailcall]
    pub fn get(this: &Self, id: usize) -> Option<&Capability> {
        let offset = id % NUM_NODES_PER_ENTRY;
        let id = id / NUM_NODES_PER_ENTRY;
        let node = &this.nodes[offset];
        if id == 0 {
            Some(&node.capability)
        } else {
            let child = unsafe { this.nodes[offset].child?.as_ref() };
            Self::get(child, id)
        }
    }

    pub fn insert(&mut self, offset: usize, capability: Capability) -> Result<(), Capability> {
        todo!()
    }

    pub fn delete(&mut self, offset: usize) -> Option<Capability> {
        todo!();
    }

    pub fn link(&mut self, offset: usize, entry: NonNull<CapabilityEntry>) {
        todo!();
    }

    pub fn unlink(&mut self, offset: usize) -> Option<NonNull<CapabilityEntry>> {
        todo!();
    }
}

impl CapabilityNode {
    pub fn empty() -> Self {
        Self {
            capability: Capability::Empty,
            child: None,
        }
    }
}

const _SIZE_AND_ALIGNMENT_REQUIRED: () = {
    assert!(core::mem::size_of::<CapabilityEntry>() == 4096);
    assert!(core::mem::align_of::<CapabilityEntry>() == 4096);
    assert!(core::mem::size_of::<CapabilityNode>() == 64);
    assert!(core::mem::align_of::<CapabilityNode>() == 64);
};
