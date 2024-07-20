//! Higher-level structured syscalls
//!
//! This module provides a higher-level set of operations to raw
//! kernel APIs.

use self::structures::{CapTable, HardwareAccess, PageTable, Retype, SyncRet, Thread};
use crate::raw::CapId;

pub mod cap_managment;
pub mod structures;
pub mod sync_call;

pub struct Booter {
    pub sync_ret: SyncRet,
    pub retype: Retype,
    pub self_caps: CapTable,
    pub self_thread: Thread,
    pub self_paging: PageTable,
    pub hardware: HardwareAccess,
}
impl Booter {
    pub const fn make() -> Self {
        Self {
            sync_ret: SyncRet::new(CapId::new(0)),
            retype: Retype::new(CapId::new(1)),
            self_caps: CapTable::new(CapId::new(2)),
            self_thread: Thread::new(CapId::new(3)),
            self_paging: PageTable::new(CapId::new(4)),
            hardware: HardwareAccess::new(CapId::new(5)),
        }
    }
}
