#[cfg(target_arch = "x86_64")]
pub mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

use core::{
    cell::Ref,
    sync::atomic::{AtomicU16, Ordering},
};

use self::addr::Frame;

#[repr(transparent)]
pub struct RetypeEntry(AtomicU16);

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[repr(u8)]
pub enum RetypeState {
    Unavailable = 0,
    Untyped = 1,
    User = 2,
    Kernel = 3,
}

#[derive(Debug)]
pub struct ReadEntry {
    pub state: RetypeState,
    pub ref_count: u16,
}

impl RetypeEntry {
    const STATE_BITS: u16 = 2;
    const COUNTER_BITS: u16 = 16 - Self::STATE_BITS;
    pub const MAX_REF_COUNT: u16 = (1 << Self::COUNTER_BITS) - 1;

    const fn value_into(value: u16) -> ReadEntry {
        let ref_count = value & ((1 << Self::COUNTER_BITS) - 1);
        let state = match RetypeState::try_from((value >> Self::COUNTER_BITS) as u8) {
            Ok(state) => state,
            Err(_e) => panic!("Invalid retype state"),
        };
        ReadEntry { state, ref_count }
    }

    pub fn get(&self) -> ReadEntry {
        Self::value_into(self.0.load(Ordering::Relaxed))
    }
}

#[derive(Debug)]
struct Invalid;
impl RetypeState {
    const fn try_from(value: u8) -> Result<Self, Invalid> {
        match value {
            0 => Ok(Self::Unavailable),
            1 => Ok(Self::Untyped),
            2 => Ok(Self::User),
            3 => Ok(Self::Kernel),
            _ => Err(Invalid),
        }
    }
}

impl core::fmt::Debug for RetypeEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.get().fmt(f)
    }
}

#[repr(transparent)]
pub struct RetypeTable<'a> {
    table: &'a [RetypeEntry],
}

impl RetypeTable<'_> {
    pub fn get(&self, frame: usize) -> Option<(ReadEntry, Frame)> {
        let state = self.table.get(frame)?.get();
        let frame = Frame::from_index(frame as u64)
            .expect("The table is large enough to include the index, so the frame should be valid");
        Some((state, frame))
    }

    pub fn iter(&self) -> impl Iterator<Item = (ReadEntry, Frame)> + '_ {
        let mut index = 0;
        let length = self.table.len();
        core::iter::from_fn(move || {
            let out = self.get(index);
            index += 1;
            out
        })
    }
}
