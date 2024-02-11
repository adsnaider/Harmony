use elain::Align;

use crate::arch::mm::addrspace::AddrSpace;
use crate::arch::PAGE_SIZE;
use crate::capabilities::trie::CapabilityTable;

pub struct Component {
    memory: AddrSpace,
    capabilities: CapabilityTable,
    _align: Align<PAGE_SIZE>,
}
