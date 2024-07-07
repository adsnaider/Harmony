//! Higher-level structured syscalls
//!
//! This module provides a higher-level set of operations to raw
//! kernel APIs.

use kapi::ops::cap_table::{CapTableOp, ConsArgs, ConstructArgs, SlotId, ThreadConsArgs};
use kapi::ops::hardware::HardwareOp;
use kapi::ops::thread::ThreadOp;
use kapi::ops::SyscallOp as _;
use kapi::raw::{CapError, CapId};

pub struct PhysFrame(usize);

impl PhysFrame {
    pub fn new(frame: usize) -> Self {
        assert!(frame % 4096 == 0);
        Self(frame)
    }
}

/// A wrapper over a capability table capability.
#[derive(Debug, Clone, Copy)]
pub struct CapTable {
    id: CapId,
}

impl CapTable {
    pub fn new(cap: CapId) -> Self {
        Self { id: cap }
    }

    /// Constructs
    pub unsafe fn make_thread(
        &self,
        entry: extern "C" fn(usize) -> !,
        stack_top: *mut u8,
        resources: CapTable,
        page_table: PageTable,
        construct_slot: SlotId<128>,
        construct_frame: PhysFrame,
        arg0: usize,
    ) -> Result<(), CapError> {
        let op = CapTableOp::Construct(ConsArgs {
            kind: ConstructArgs::Thread(ThreadConsArgs {
                entry: entry as usize,
                stack_pointer: stack_top as usize,
                cap_table: resources.id,
                page_table: page_table.id,
                arg0,
            }),
            region: construct_frame.0,
            slot: construct_slot,
        });
        unsafe {
            op.syscall(self.id)?;
        }
        Ok(())
    }
}

/// A wrapper over a thread capability.
#[derive(Debug, Clone, Copy)]
pub struct Thread {
    id: CapId,
}

impl Thread {
    pub fn new(id: CapId) -> Self {
        Self { id }
    }

    pub unsafe fn activate(&self) -> Result<(), CapError> {
        ThreadOp::Activate.syscall(self.id)?;
        crate::println!("Came back");
        Ok(())
    }
}

/// A wrapper over a page table capability.
#[derive(Debug, Clone, Copy)]
pub struct PageTable {
    id: CapId,
}

impl PageTable {
    pub fn new(id: CapId) -> Self {
        Self { id }
    }
}

pub struct HardwareAccess {
    id: CapId,
}

impl HardwareAccess {
    pub fn new(id: CapId) -> Self {
        Self { id }
    }

    pub fn enable_ports(&self) -> Result<(), CapError> {
        unsafe { HardwareOp::EnableIoPorts.syscall(self.id) }?;
        Ok(())
    }
}
