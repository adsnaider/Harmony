//! Higher-level structured syscalls
//!
//! This module provides a higher-level set of operations to raw
//! kernel APIs.

use core::convert::Infallible;

use crate::ops::cap_table::{
    CapTableOp, ConsArgs, ConstructArgs, SlotId, SyncCallConsArgs, ThreadConsArgs,
};
use crate::ops::hardware::HardwareOp;
use crate::ops::ipc::{SyncCallOp, SyncRetOp};
use crate::ops::memory::{RetypeKind, RetypeOp};
use crate::ops::thread::ThreadOp;
use crate::ops::SyscallOp as _;
use crate::raw::{CapError, CapId};

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
                region: construct_frame.0,
            }),
            slot: construct_slot,
        });
        unsafe { op.syscall(self.id) }
    }

    pub unsafe fn make_sync_call(
        &self,
        entry: extern "C" fn(usize, usize, usize, usize) -> usize,
        resources: CapTable,
        page_table: PageTable,
        construct_slot: SlotId<128>,
    ) -> Result<(), CapError> {
        let op = CapTableOp::Construct(ConsArgs {
            kind: ConstructArgs::SyncCall(SyncCallConsArgs {
                entry: entry as usize,
                cap_table: resources.id,
                page_table: page_table.id,
            }),
            slot: construct_slot,
        });
        unsafe { op.syscall(self.id) }
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
        unsafe { ThreadOp::Activate.syscall(self.id) }
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
        unsafe { HardwareOp::EnableIoPorts.syscall(self.id) }
    }
}

pub struct SyncCall {
    id: CapId,
}

impl SyncCall {
    pub const fn new(id: CapId) -> Self {
        Self { id }
    }

    pub fn call(&self, a: usize, b: usize, c: usize, d: usize) -> Result<usize, CapError> {
        unsafe { SyncCallOp::Call((a, b, c, d)).syscall(self.id) }
    }
}

pub struct SyncRet {
    id: CapId,
}

impl SyncRet {
    pub const fn new(id: CapId) -> Self {
        Self { id }
    }

    pub fn ret(&self, code: usize) -> Result<Infallible, CapError> {
        unsafe { SyncRetOp::SyncRet(code).syscall(self.id) }
    }
}

pub struct Retype {
    id: CapId,
}

impl Retype {
    pub const fn new(id: CapId) -> Self {
        Self { id }
    }

    pub fn retype(&self, region: usize, kind: RetypeKind) -> Result<(), CapError> {
        unsafe { RetypeOp { region, to: kind }.syscall(self.id) }
    }
}
