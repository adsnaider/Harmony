use crate::arch::mm::addrspace::AddrSpace;
use crate::capabilities::CapabilityTable;

pub struct Component {
    memory: AddrSpace,
    capabilities: CapabilityTable,
}
