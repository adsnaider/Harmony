//! This module provides the raw(er) capability structures while providing abstractions to perform
//! syscalls.

use core::convert::Infallible;

use super::paging::addr::Frame;
use crate::ops::cap_table::{
    CapTableConsArgs, CapTableOp, ConsArgs, ConstructArgs, PageTableConsArgs, SyncCallConsArgs,
    ThreadConsArgs,
};
use crate::ops::hardware::HardwareOp;
use crate::ops::ipc::{SyncCallOp, SyncRetOp};
use crate::ops::memory::{RetypeKind, RetypeOp};
use crate::ops::paging::{PageTableOp, PermissionMask};
use crate::ops::thread::ThreadOp;
use crate::ops::{SlotId, SyscallOp as _};
use crate::raw::{CapError, CapId};

/// A wrapper over a capability table capability.
#[derive(Debug, Clone, Copy)]
pub struct CapTable {
    id: CapId,
}

impl CapTable {
    pub const fn new(cap: CapId) -> Self {
        Self { id: cap }
    }

    pub fn drop_resource(&self, slot: SlotId) -> Result<(), CapError> {
        let op = CapTableOp::Drop { slot };
        unsafe { op.syscall(self.id) }
    }

    pub fn make_resource(&self, args: ConstructArgs, slot: SlotId) -> Result<(), CapError> {
        let op = CapTableOp::Construct(ConsArgs { kind: args, slot });
        unsafe { op.syscall(self.id) }
    }

    pub fn copy_resource(
        &self,
        slot: SlotId,
        to_table: CapTable,
        to_slot: SlotId,
    ) -> Result<(), CapError> {
        let op = CapTableOp::Copy {
            slot,
            other_table_cap: to_table.id,
            other_slot: to_slot,
        };
        unsafe { op.syscall(self.id) }
    }

    /// Constructs
    pub fn make_thread(
        &self,
        entry: extern "C" fn(usize) -> !,
        stack_top: *mut u8,
        resources: CapTable,
        page_table: PageTable,
        construct_slot: SlotId,
        construct_frame: Frame,
        arg0: usize,
    ) -> Result<(), CapError> {
        let op = CapTableOp::Construct(ConsArgs {
            kind: ConstructArgs::Thread(ThreadConsArgs {
                entry: entry as usize,
                stack_pointer: stack_top as usize,
                cap_table: resources.id,
                page_table: page_table.id,
                arg0,
                region: construct_frame.addr().into(),
            }),
            slot: construct_slot,
        });
        unsafe { op.syscall(self.id) }
    }

    pub unsafe fn make_sync_call(
        &self,
        entry: extern "C" fn(usize, usize, usize, usize) -> isize,
        resources: CapTable,
        page_table: PageTable,
        construct_slot: SlotId,
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

    pub fn make_page_table(&self, slot: SlotId, frame: Frame, level: u8) -> Result<(), CapError> {
        unsafe {
            CapTableOp::Construct(ConsArgs {
                kind: ConstructArgs::PageTable(PageTableConsArgs {
                    region: frame.addr().into(),
                    level,
                    _padding: [0; 7],
                }),
                slot,
            })
            .syscall(self.id)
        }
    }

    pub fn make_cap_table(&self, slot: SlotId, frame: Frame) -> Result<(), CapError> {
        unsafe {
            CapTableOp::Construct(ConsArgs {
                kind: ConstructArgs::CapTable(CapTableConsArgs {
                    region: frame.addr().into(),
                }),
                slot,
            })
            .syscall(self.id)
        }
    }

    pub fn link_table(&self, slot: SlotId, table: CapTable) -> Result<(), CapError> {
        unsafe {
            CapTableOp::Link {
                slot,
                other_table_cap: table.id,
            }
            .syscall(self.id)
        }
    }
}

/// A wrapper over a thread capability.
#[derive(Debug, Clone, Copy)]
pub struct Thread {
    id: CapId,
}

impl Thread {
    pub const fn new(id: CapId) -> Self {
        Self { id }
    }

    pub unsafe fn activate(&self) -> Result<(), CapError> {
        unsafe { ThreadOp::Activate.syscall(self.id) }
    }
}

/// A wrapper over a page table capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageTable {
    id: CapId,
}

impl PageTable {
    pub const fn new(id: CapId) -> Self {
        Self { id }
    }

    pub fn link(
        &self,
        other_table: PageTable,
        slot: usize,
        permissions: PermissionMask,
    ) -> Result<(), CapError> {
        unsafe {
            PageTableOp::Link {
                other_table: other_table.id,
                slot,
                permissions,
            }
            .syscall(self.id)
        }
    }

    pub fn map(
        &self,
        slot: usize,
        frame: Frame,
        permissions: PermissionMask,
    ) -> Result<(), CapError> {
        unsafe {
            PageTableOp::MapFrame {
                user_frame: frame.addr().into(),
                slot,
                permissions,
            }
            .syscall(self.id)
        }
    }

    pub fn unlink(&self, slot: usize) -> Result<(), CapError> {
        unsafe { PageTableOp::Unlink { slot }.syscall(self.id) }
    }

    pub fn unmap(&self, slot: usize) -> Result<(), CapError> {
        unsafe { PageTableOp::UnmapFrame { slot }.syscall(self.id) }
    }
}

pub struct HardwareAccess {
    id: CapId,
}

impl HardwareAccess {
    pub const fn new(id: CapId) -> Self {
        Self { id }
    }

    pub fn enable_ports(&self) -> Result<(), CapError> {
        unsafe { HardwareOp::EnableIoPorts.syscall(self.id) }
    }

    pub fn flush_page(&self, page: usize) -> Result<(), CapError> {
        unsafe { HardwareOp::FlushPage { addr: page }.syscall(self.id) }
    }
}

pub struct SyncCall {
    id: CapId,
}

impl SyncCall {
    pub const fn new(id: CapId) -> Self {
        Self { id }
    }

    pub fn call(&self, a: usize, b: usize, c: usize, d: usize) -> Result<isize, CapError> {
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

    pub fn retype(&self, frame: Frame, kind: RetypeKind) -> Result<(), CapError> {
        unsafe {
            RetypeOp {
                region: frame.addr().into(),
                to: kind,
            }
            .syscall(self.id)
        }
    }
}

impl CapTableConsArgs {
    pub fn new(frame: Frame) -> Self {
        Self {
            region: frame.addr().into(),
        }
    }
}
impl ThreadConsArgs {
    pub fn new(
        entry: extern "C" fn(usize) -> !,
        stack_top: *mut u8,
        resources: CapTable,
        page_table: PageTable,
        construct_frame: Frame,
        arg0: usize,
    ) -> Self {
        Self {
            entry: entry as usize,
            stack_pointer: stack_top as usize,
            cap_table: resources.id,
            page_table: page_table.id,
            arg0,
            region: construct_frame.addr().into(),
        }
    }
}

impl PageTableConsArgs {
    pub fn new(frame: Frame, level: u8) -> Self {
        Self {
            region: frame.addr().into(),
            level,
            _padding: [0; 7],
        }
    }
}

impl SyncCallConsArgs {
    pub fn new(
        entry: extern "C" fn(usize, usize, usize, usize) -> isize,
        resources: CapTable,
        page_table: PageTable,
    ) -> Self {
        Self {
            entry: entry as usize,
            cap_table: resources.id,
            page_table: page_table.id,
        }
    }
}
