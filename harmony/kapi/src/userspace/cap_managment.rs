use super::structures::{CapTable, PageTable, PhysFrame, SyncCall, Thread};
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
    current_cap: CapId,
    frame_allocator: A,
}

pub trait FrameAllocator {
    fn alloc_frame(&mut self) -> PhysFrame;
}

pub struct CapSlot {
    id: CapId,
    table: CapTable,
    slot: SlotId,
}

impl CapSlot {
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
}

impl<A: FrameAllocator> SelfCapabilityManager<A> {
    /// Returns a new capability manager, assuming no capabilities are set.
    pub const fn new(root_table: CapTable, allocator: A) -> Self {
        Self {
            root_table,
            current_cap: CapId::new(0),
            frame_allocator: allocator,
        }
    }

    /// Returns a new capability manager that will not attempt to set any capabilities smaller than `start_cap`.
    pub const fn new_with_start(root_table: CapTable, start_cap: CapId, allocator: A) -> Self {
        Self {
            root_table,
            current_cap: start_cap,
            frame_allocator: allocator,
        }
    }

    pub fn allocate_capability(&mut self) -> CapSlot {
        let cap = self.current_cap;
        self.current_cap = CapId::new(self.current_cap.get() + 1);
        // If we reach the end of the table, it's time to allocate a new capability table
        // to hold more capabilities. The last slot on each table is then reserved for
        // managing page tables.
        if cap.get() % SlotId::count() as u32 == SlotId::count() as u32 - 1 {
            let table_cap = cap;
            let new_cap = self.current_cap;
            self.current_cap = CapId::new(self.current_cap.get() + 1);
            //TODO: Allocate a new table
            let frame = self.frame_allocator.alloc_frame();
            let parent_table = self.table_for_cap(table_cap);
            parent_table
                .make_cap_table(SlotId::new(SlotId::count() - 1).unwrap(), frame)
                .unwrap();

            let table = CapTable::new(table_cap);
            CapSlot {
                id: new_cap,
                table,
                slot: SlotId::new(0).unwrap(),
            }
        } else {
            CapSlot {
                id: cap,
                table: self.root_table,
                slot: (cap.get() as usize).try_into().unwrap(),
            }
        }
    }

    fn table_for_cap(&self, cap: CapId) -> CapTable {
        // The following holds that given a slot count S,
        // any capability C >= S, will be managed by the table allocated in Ct = (C / S) * S - 1
        // and if C < S, Ct = CTroot
        let cap = cap.get();
        if cap < SlotId::count() as u32 {
            self.root_table
        } else {
            CapTable::new(CapId::new(
                (cap / SlotId::count() as u32) * SlotId::count() as u32 - 1,
            ))
        }
    }
}
