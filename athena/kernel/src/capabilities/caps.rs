use super::trie::CapabilityTable;

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum Capability {
    Empty,
    Thread,
    ThreadCreate,
    CapTable(CapabilityTable),
    PageTable,
    MemoryTyping,
}
