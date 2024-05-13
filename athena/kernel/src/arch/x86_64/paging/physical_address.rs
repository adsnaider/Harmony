#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PhysAddr {
    addr: u64,
}

#[derive(Debug)]
pub struct BadAddress;

impl PhysAddr {
    pub fn new(addr: u64) -> Self {
        Self::try_new(addr).unwrap()
    }

    pub fn try_new(addr: u64) -> Result<Self, BadAddress> {
        if Self::new_truncate(addr).addr == addr {
            Ok(Self { addr })
        } else {
            Err(BadAddress)
        }
    }

    pub fn new_truncate(addr: u64) -> Self {
        Self {
            addr: addr % (1 << 52),
        }
    }

    pub fn as_u64(&self) -> u64 {
        self.addr
    }
}
