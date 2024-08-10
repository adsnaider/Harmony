use super::virtual_address::BadVirtAddr;
use super::{VirtAddr, PAGE_SIZE};

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct Page {
    start_address: VirtAddr,
}

#[derive(Debug)]
pub struct Unaligned;

impl Page {
    pub const fn from_start_address(addr: VirtAddr) -> Self {
        match Self::try_from_start_address(addr) {
            Err(Unaligned) => panic!("Unaligned start address"),
            Ok(this) => this,
        }
    }

    pub const fn size() -> usize {
        PAGE_SIZE
    }

    pub const fn try_from_start_address(addr: VirtAddr) -> Result<Self, Unaligned> {
        if addr.as_usize() % PAGE_SIZE != 0 {
            return Err(Unaligned);
        }
        Ok(Self {
            start_address: addr,
        })
    }

    pub fn containing_address(addr: VirtAddr) -> Self {
        let addr = VirtAddr::new((addr.as_usize() / PAGE_SIZE) * PAGE_SIZE);
        Self::from_start_address(addr)
    }

    pub fn base(&self) -> VirtAddr {
        self.start_address
    }

    /// Returns the page defined by `base = index * PAGE_SIZE`
    pub const fn from_index(index: usize) -> Result<Self, BadVirtAddr> {
        match VirtAddr::try_new(index * PAGE_SIZE) {
            Ok(addr) => Ok(Self::from_start_address(addr)),
            Err(e) => Err(e),
        }
    }

    /// Returns the index of this page (inverse of `from_index`)
    pub const fn index(&self) -> usize {
        self.start_address.as_usize() / PAGE_SIZE
    }
}
