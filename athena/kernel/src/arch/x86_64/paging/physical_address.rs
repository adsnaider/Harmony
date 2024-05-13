use super::VirtAddr;
use crate::PMO;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PhysAddr(u64);

#[derive(Debug)]
pub struct BadAddress;

impl PhysAddr {
    pub const fn new(addr: u64) -> Self {
        match Self::try_new(addr) {
            Ok(addr) => addr,
            Err(_) => panic!("Invalid Physical Address: Must be up to 52 bits"),
        }
    }

    pub const fn try_new(addr: u64) -> Result<Self, BadAddress> {
        if Self::new_truncate(addr).0 == addr {
            Ok(Self(addr))
        } else {
            Err(BadAddress)
        }
    }

    pub const fn new_truncate(addr: u64) -> Self {
        Self(addr % (1 << 52))
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn to_virtual(self) -> VirtAddr {
        let virt = PMO.get().as_usize() + self.0 as usize;
        VirtAddr::new(virt)
    }
}
