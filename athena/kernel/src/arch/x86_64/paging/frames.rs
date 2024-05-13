//! Physical memory frames for the x86-64 architecture

use super::PAGE_SIZE;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct RawFrame {
    base: u64,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum InvalidFrame {
    BadAddress,
    UnalignedAddress,
}

impl RawFrame {
    pub fn from_start_address(base: u64) -> Self {
        Self::try_from_start_address(base).unwrap()
    }

    pub fn try_from_start_address(base: u64) -> Result<Self, InvalidFrame> {
        if base % PAGE_SIZE as u64 != 0 {
            return Err(InvalidFrame::UnalignedAddress);
        }
        if base % (1 << 52) != base {
            return Err(InvalidFrame::BadAddress);
        }
        Ok(Self { base })
    }
}
