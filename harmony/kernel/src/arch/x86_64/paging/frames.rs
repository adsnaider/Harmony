//! Physical memory frames for the x86-64 architecture

use x86_64_impl::structures::paging::PhysFrame;

use super::physical_address::BadAddress;
use super::{PhysAddr, FRAME_SIZE};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct RawFrame {
    base: PhysAddr,
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

    pub fn from_index(index: u64) -> Result<Self, BadAddress> {
        let start = index * FRAME_SIZE;
        Ok(Self::from_start_address(PhysAddr::try_new(start)?))
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

impl From<RawFrame> for PhysFrame {
    fn from(value: RawFrame) -> Self {
        unsafe {
            PhysFrame::from_start_address_unchecked(x86_64_impl::PhysAddr::new_unsafe(
                value.base.as_u64(),
            ))
        }
    }
}
