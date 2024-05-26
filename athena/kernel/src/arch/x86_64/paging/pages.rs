use super::{VirtAddr, PAGE_SIZE};

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct Page {
    start_address: VirtAddr,
}

#[derive(Debug)]
pub struct Unaligned;

impl Page {
    pub fn from_start_address(addr: VirtAddr) -> Self {
        Self::try_from_start_address(addr).unwrap()
    }

    pub fn try_from_start_address(addr: VirtAddr) -> Result<Self, Unaligned> {
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
}
