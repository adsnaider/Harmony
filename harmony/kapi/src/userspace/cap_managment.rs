use super::structures::{CapTable, PageTable, SyncCall, Thread};
use crate::ops::cap_table::{
    CapTableConsArgs, ConstructArgs, PageTableConsArgs, SyncCallConsArgs, ThreadConsArgs,
};
use crate::ops::SlotId;
use crate::raw::{CapError, CapId};

/// A capability manager to handle capability tries for itself.
#[derive(Debug)]
pub struct SelfCapabilityManager {
    root_table: CapTable,
    current_cap: CapId,
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
}

impl SelfCapabilityManager {
    /// Returns a new capability manager, assuming no capabilities are set.
    pub const fn new(root_table: CapTable) -> Self {
        Self {
            root_table,
            current_cap: CapId::new(0),
        }
    }

    /// Returns a new capability manager that will not attempt to set any capabilities smaller than `start_cap`.
    pub const fn new_with_start(root_table: CapTable, start_cap: CapId) -> Self {
        Self {
            root_table,
            current_cap: start_cap,
        }
    }

    pub fn allocate_capability(&mut self) -> CapSlot {
        let cap = self.current_cap;
        self.current_cap = CapId::new(self.current_cap.get() + 1);
        if cap.get() % SlotId::count() as u32 == 0 {
            todo!();
        }
        CapSlot {
            id: cap,
            table: self.root_table,
            slot: (cap.get() as usize).try_into().unwrap(),
        }
    }

    pub fn deallocate_capaiblity(&mut self, _slot: CapSlot) {
        todo!();
    }
}
