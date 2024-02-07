use crate::arch::mm::addrspace::AddrSpace;
use crate::capabilities::trie::CapabilityTable;

pub struct Component {
    memory: AddrSpace,
    capabilities: CapabilityTable,
}
