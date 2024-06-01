//! Physical memory frames for the x86-64 architecture

use super::{PhysAddr, FRAME_SIZE};

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct RawFrame {
    base: PhysAddr,
}

impl core::fmt::Debug for RawFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "RawFrame({:#X})", self.base.as_u64())
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct UnalignedAddress;

impl RawFrame {
    pub fn from_start_address(base: PhysAddr) -> Self {
        Self::try_from_start_address(base).unwrap()
    }

    pub fn base(&self) -> PhysAddr {
        self.base
    }

    pub fn try_from_start_address(base: PhysAddr) -> Result<Self, UnalignedAddress> {
        if base.as_u64() % FRAME_SIZE != 0 {
            return Err(UnalignedAddress);
        }
        Ok(Self { base })
    }

    pub fn within_frame(addr: PhysAddr) -> Self {
        let base = PhysAddr::new(addr.as_u64() % FRAME_SIZE);
        Self { base }
    }

    pub fn addr(&self) -> PhysAddr {
        self.base
    }
}
