use core::ops::{Add, AddAssign, Sub, SubAssign};

use super::paging::addr::Frame;
use super::structures::{CapTable, PageTable, SyncCall, Thread};
use crate::ops::cap_table::{
    CapTableConsArgs, ConstructArgs, PageTableConsArgs, SyncCallConsArgs, ThreadConsArgs,
};
use crate::ops::SlotId;
use crate::raw::{CapError, CapId};

/// A capability manager to handle capability tries for itself.
///
/// As capability tables are filled up, this class will use one of the slots
/// in the trie to manage the capability to further capability tables.
#[derive(Debug)]
pub struct SelfCapabilityManager<A: FrameAllocator> {
    root_table: CapTable,
    cap_generator: CapGenerator,
    frame_allocator: A,
}

pub trait FrameAllocator {
    fn alloc_frame(&mut self) -> Frame;
}

/// A slot into a self-managed capability table
///
/// This struct holds:
/// 1. A capability ID to a resource, and
/// 2. A capability to the leaf table and the slot that holds the resource itself
///
/// This enables modifying this particular slot, creating resources, dropping them, or copying them in/out.
#[derive(Debug)]
pub struct CapSlot {
    id: CapId,
    table: CapTable,
    slot: SlotId,
}

impl CapSlot {
    fn from_root_and_cap(root_table: CapTable, cap: CapId) -> Self {
        // Each capability will be managed by the pervious table's last element.
        let cap_idx: CapIndex = cap.into();
        let cap_idx = cap_idx.as_u32();

        // Strip the last SlotId::bits() to get to the start of the table and subtract one to get
        // to the managed table.
        const MASK: u32 = !(SlotId::count() as u32 - 1);
        let table = if cap_idx & MASK == 0 {
            root_table
        } else {
            let table_cap: CapId = CapIndex::from_idx((cap_idx & MASK) - 1).try_into().expect("Only 0-slot capabilities may be invalid. This should always point to the end of a table");
            CapTable::new(table_cap)
        };

        let slot = SlotId::new(cap_idx as usize % SlotId::count())
            .expect("Modulo arithmetic guarantees this will be within bounds");
        Self {
            id: cap,
            table,
            slot,
        }
    }

    pub fn cap(&self) -> CapId {
        self.id
    }

    pub fn make_resource(&self, args: ConstructArgs) -> Result<(), CapError> {
        self.table.make_resource(args, self.slot)
    }

    pub fn make_thread(&self, thread_args: ThreadConsArgs) -> Result<Thread, CapError> {
        self.make_resource(ConstructArgs::Thread(thread_args))?;
        Ok(Thread::new(self.id))
    }

    pub fn make_cap_table(&self, args: CapTableConsArgs) -> Result<CapTable, CapError> {
        self.make_resource(ConstructArgs::CapTable(args))?;
        Ok(CapTable::new(self.id))
    }

    pub fn make_page_table(&self, args: PageTableConsArgs) -> Result<PageTable, CapError> {
        self.make_resource(ConstructArgs::PageTable(args))?;
        Ok(PageTable::new(self.id))
    }

    pub fn make_sync_call(&self, args: SyncCallConsArgs) -> Result<SyncCall, CapError> {
        self.make_resource(ConstructArgs::SyncCall(args))?;
        Ok(SyncCall::new(self.id))
    }

    pub fn deallocate(self) -> Result<(), CapError> {
        self.table.drop_resource(self.slot)
    }

    pub fn copy_into(&self, to_table: CapTable, to_slot: SlotId) -> Result<(), CapError> {
        self.table.copy_resource(self.slot, to_table, to_slot)
    }
}

#[derive(Debug)]
struct CapGenerator {
    index: CapIndex,
}

#[derive(Debug)]
struct NoCapForIndex;

/// A capability index that provides sensible capability IDs when used as a counter.
///
/// This type is a tricky inverse of a [`CapId`]. It's possible to convert a [`CapId`]
/// into this type and viceversa. The conversion is useful in order to generate capability
/// tables that grow in a breadth first manner.
#[derive(Debug, Copy, Clone)]
struct CapIndex(u32);

impl Default for CapIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl CapIndex {
    pub const fn new() -> Self {
        Self(0)
    }

    pub const fn from_idx(idx: u32) -> Self {
        Self(idx)
    }

    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl Add<u32> for CapIndex {
    type Output = Self;

    fn add(self, rhs: u32) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign<u32> for CapIndex {
    fn add_assign(&mut self, rhs: u32) {
        *self = *self + rhs
    }
}

impl Sub<u32> for CapIndex {
    type Output = Self;

    fn sub(self, rhs: u32) -> Self::Output {
        Self(self.0 - rhs)
    }
}

impl SubAssign<u32> for CapIndex {
    fn sub_assign(&mut self, rhs: u32) {
        *self = *self - rhs
    }
}

impl From<CapId> for CapIndex {
    fn from(cap: CapId) -> Self {
        // This works by computing the offsets of the capability and effectively
        // flipping them to rebuild the index. This is because in capability tries,
        // the LSBs index into the root, followed by the next set of bits and so on,
        // but in order to have that work as an index, we need the root to be the MSB
        // and the leaves the LSBs.
        let offsets = cap.offsets();
        let leading_zeros = offsets
            .iter()
            .rev()
            .position(|&off| off != 0)
            .unwrap_or(offsets.len());
        let valid_offsets = offsets.len() - leading_zeros;
        let mut index = 0;
        for offset in offsets.iter().take(valid_offsets) {
            index <<= SlotId::bits();
            index |= offset;
        }
        Self::from_idx(index)
    }
}

impl TryFrom<CapIndex> for CapId {
    type Error = NoCapForIndex;

    fn try_from(CapIndex(idx): CapIndex) -> Result<Self, Self::Error> {
        // This works by computing the offsets of the index and effectively
        // flipping them to build the capability. This is because in capability tries,
        // the LSBs index into the root, followed by the next set of bits and so on,
        // but in order to have that work as an index, we need the root to be the MSB
        // and the leaves the LSBs.
        //
        // One important caveat is that this doesn't always work. In particular, it's never
        // possible to index into the 0th slot of any table as that would be the exact same
        // value as indexing into the higher level table's offset (since all the following
        // bits are 0). In that case, this will return an error as an invalid Index -> Cap
        // conversion.
        let offsets = CapId::new(idx).offsets();
        let leading_zeros = offsets
            .iter()
            .rev()
            .position(|&off| off != 0)
            .unwrap_or(offsets.len());
        let valid_offsets = offsets.len() - leading_zeros;
        let mut cap = 0;
        if offsets[0] == 0 {
            return Err(NoCapForIndex);
        }
        for offset in offsets.iter().take(valid_offsets) {
            cap <<= SlotId::bits();
            cap |= offset;
        }
        Ok(CapId::new(cap))
    }
}

impl CapGenerator {
    pub const fn new() -> Self {
        Self {
            index: CapIndex::new(),
        }
    }

    pub fn from_starting_cap(cap: CapId) -> Self {
        let index = CapIndex::from(cap);
        Self { index }
    }

    pub fn get_next(&mut self) -> CapAllocation {
        let mut next_cap_gen = || {
            let cap: CapId = loop {
                if let Ok(cap) = self.index.try_into() {
                    self.index += 1;
                    break cap;
                }
                self.index += 1;
            };
            cap
        };

        let cap = next_cap_gen();
        if cap
            .offsets()
            .into_iter()
            .rev()
            .find(|&off| off != 0)
            .is_some_and(|off| off == SlotId::count() as u32 - 1)
        {
            let table_cap = cap.into();
            let cap = next_cap_gen().into();
            CapAllocation::RequiresTable {
                table_cap,
                new_cap: cap,
            }
        } else {
            CapAllocation::Simple(cap.into())
        }
    }
}

enum CapAllocation {
    Simple(CapId),
    RequiresTable { table_cap: CapId, new_cap: CapId },
>>>>>>> Stashed changes
}

impl<A: FrameAllocator> SelfCapabilityManager<A> {
    /// Returns a new capability manager, assuming no capabilities are set.
    pub const fn new(root_table: CapTable, allocator: A) -> Self {
        Self {
            root_table,
            cap_generator: CapGenerator::new(),
            frame_allocator: allocator,
        }
    }

    pub fn root(&self) -> &CapTable {
        &self.root_table
    }

    /// Returns a new capability manager that will not attempt to set any capabilities smaller than `start_cap`.
    pub fn new_with_start(root_table: CapTable, start_cap: CapId, allocator: A) -> Self {
        Self {
            root_table,
            cap_generator: CapGenerator::from_starting_cap(start_cap),
            frame_allocator: allocator,
        }
    }

    pub fn allocate_capability(&mut self) -> Result<CapSlot, CapError> {
        match self.cap_generator.get_next() {
            CapAllocation::Simple(cap) => Ok(CapSlot::from_root_and_cap(self.root_table, cap)),
            CapAllocation::RequiresTable { table_cap, new_cap } => {
                {
                    let frame = self.frame_allocator.alloc_frame();
                    let parent_slot = CapSlot::from_root_and_cap(self.root_table, table_cap);
                    parent_slot.table.make_cap_table(SlotId::tail(), frame)?;
                }
                let table = CapTable::new(table_cap);
                // Link the table to its parent
                let parent_slot = {
                    let mut offsets = new_cap.offsets();
                    if let Some(trailing) = offsets.iter_mut().rev().find(|x| **x != 0) {
                        *trailing = 0;
                    }
                    let parent = CapId::from_offsets(offsets);
                    CapSlot::from_root_and_cap(self.root_table, parent)
                };
                parent_slot
                    .table
                    .link_table(parent_slot.slot, table)
                    .unwrap();

                Ok(CapSlot {
                    id: new_cap,
                    table,
                    slot: SlotId::new(new_cap.get() as usize >> 6).unwrap(),
                })
            }
        }
    }
}
