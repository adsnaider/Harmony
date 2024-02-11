use super::trie::CapabilityTable;

#[derive(Debug, Clone)]
#[repr(C)]
pub enum Capability {
    Empty,
    Thread,
    ThreadCreate,
    CapTable(CapabilityTable),
    PageTable,
    MemoryTyping,
}
