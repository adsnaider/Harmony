use super::trie::CapabilityTable;
use crate::arch::mm::retyping::MemoryBlock;
use crate::thread::Tid;

#[derive(Debug, Clone)]
#[repr(C)]
pub enum Capability {
    Empty,
    Thread(Tid),
    ThreadCreate,
    CapTable(CapabilityTable),
    PageTable,
    MemoryTyping(MemoryBlock),
}
