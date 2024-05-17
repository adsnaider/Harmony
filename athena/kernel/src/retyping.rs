use core::mem::{ManuallyDrop, MaybeUninit};
use core::sync::atomic::{AtomicU16, Ordering};

use limine::memory_map::EntryType;
use sync::cell::AtomicOnceCell;

use crate::arch::paging::{RawFrame, FRAME_SIZE, PAGE_SIZE};
use crate::bump_allocator::BumpAllocator;
use crate::MemoryMap;

static RETYPE_TABLE: AtomicOnceCell<RetypeTable> = AtomicOnceCell::new();

pub struct RetypeTable {
    retype_map: &'static mut [RetypeEntry],
}

impl RetypeTable {
    pub fn new(memory_map: MemoryMap) -> Option<Self> {
        let physical_top = {
            let last = memory_map.iter().last()?;
            last.base + last.length
        };
        assert!(physical_top % FRAME_SIZE == 0);
        let number_frames = (physical_top / FRAME_SIZE) as usize;
        let mut allocator = BumpAllocator::new(memory_map);

        let retype_map_frames = {
            let retype_map_size = core::mem::size_of::<RetypeEntry>() * number_frames;
            assert!(retype_map_size > 0);
            (retype_map_size - 1) / PAGE_SIZE + 1
        };
        let retype_map = {
            let start_physical_address = allocator.alloc_frames(retype_map_frames)?;
            let start_addr: *mut MaybeUninit<RetypeEntry> =
                start_physical_address.to_virtual().as_mut_ptr();
            // SAFETY: Memory is allocated and off the memory map
            unsafe { core::slice::from_raw_parts_mut(start_addr, number_frames) }
        };

        for entry in retype_map.iter_mut() {
            entry.write(RetypeEntry::unavailable());
        }
        // SAFETY: Initialized in earlier loop
        let retype_map: &mut [RetypeEntry] = unsafe { core::mem::transmute(retype_map) };

        let memory_map = allocator.into_memory_map();
        for entry in memory_map
            .iter()
            .filter(|entry| entry.entry_type == EntryType::USABLE)
        {
            assert!(entry.base % FRAME_SIZE == 0);
            assert!(entry.length % FRAME_SIZE == 0);
            let start_idx = (entry.base / FRAME_SIZE) as usize;
            let count = (entry.length / FRAME_SIZE) as usize;
            for i in start_idx..(start_idx + count) {
                retype_map[i] = RetypeEntry::untyped();
            }
        }
        Some(Self { retype_map })
    }

    pub fn set_as_global(self) -> Result<(), sync::cell::OnceError> {
        RETYPE_TABLE.set(self)
    }
}

#[derive(Debug)]
pub struct OutOfBounds;

impl From<OutOfBounds> for RetypeError {
    fn from(_value: OutOfBounds) -> Self {
        RetypeError::OutOfBounds
    }
}

impl From<OutOfBounds> for AsTypeError {
    fn from(_value: OutOfBounds) -> Self {
        AsTypeError::OutOfBounds
    }
}

#[derive(Debug)]
pub enum RetypeError {
    InvalidFromState(State),
    RefsExist(u16),
    OutOfBounds,
}

#[derive(Debug)]
pub enum AsTypeError {
    NotExpectedState(State),
    MaxRefs,
    OutOfBounds,
}

#[derive(Debug, Copy, Clone)]
pub struct MaxRefs;
#[derive(Debug, Copy, Clone)]
pub struct NoRefs;

#[repr(transparent)]
#[derive(Debug)]
pub struct UserFrame(RawFrame);

impl RawFrame {
    fn retype_entry(&self) -> Result<&'static RetypeEntry, OutOfBounds> {
        let index = (self.addr().as_u64() / FRAME_SIZE) as usize;
        RETYPE_TABLE
            .get()
            .unwrap()
            .retype_map
            .get(index)
            .ok_or(OutOfBounds)
    }

    pub fn try_as_user(self) -> Result<UserFrame, AsTypeError> {
        self.retype_entry()?
            .get_as_and_increment(State::User)
            .map_err(|(state, value)| {
                if !matches!(state, State::User) {
                    AsTypeError::NotExpectedState(state)
                } else {
                    debug_assert!(value == RetypeEntry::MAX_REF_COUNT);
                    AsTypeError::MaxRefs
                }
            })?;
        Ok(UserFrame(self))
    }

    /// Unsafely turn a raw frame into a user frame.
    ///
    /// # Safety
    ///
    /// The raw frame must be typed as user
    pub unsafe fn as_user_unchecked(self) -> UserFrame {
        let frame = UserFrame(self);
        frame.entry().increment().unwrap();
        frame
    }

    pub fn try_as_kernel(self) -> Result<KernelFrame, AsTypeError> {
        self.retype_entry()?
            .get_as_and_increment(State::Kernel)
            .map_err(|(state, value)| {
                if !matches!(state, State::User) {
                    AsTypeError::NotExpectedState(state)
                } else {
                    debug_assert!(value == RetypeEntry::MAX_REF_COUNT);
                    AsTypeError::MaxRefs
                }
            })?;
        Ok(KernelFrame(self))
    }

    /// Unsafely turn a raw frame into a kernel frame.
    ///
    /// # Safety
    ///
    /// The raw frame must be typed as kernel
    pub unsafe fn as_kernel_unchecked(self) -> KernelFrame {
        let frame = KernelFrame(self);
        frame.entry().increment().unwrap();
        frame
    }

    pub fn try_into_user(self) -> Result<UserFrame, RetypeError> {
        self.retype_entry()?
            .retype(State::Untyped, State::User, 0, 1)
            .map_err(|(state, _count)| RetypeError::InvalidFromState(state))?;
        Ok(UserFrame(self))
    }

    pub fn try_into_kernel(self) -> Result<KernelFrame, RetypeError> {
        self.retype_entry()?
            .retype(State::Untyped, State::Kernel, 0, 1)
            .map_err(|(state, _count)| RetypeError::InvalidFromState(state))?;
        Ok(KernelFrame(self))
    }

    fn try_into_untyped_from(self, from: State) -> Result<RawFrame, RetypeError> {
        assert!(matches!(from, State::User | State::Kernel));
        let entry = self.retype_entry()?;

        match entry.retype(from, State::Untyped, 0, 0) {
            Ok(()) => Ok(self),
            Err((State::Unavailable, refs)) => {
                debug_assert_eq!(refs, 0);
                Err(RetypeError::InvalidFromState(State::Unavailable))
            }
            Err((s, refs)) if s == from => {
                debug_assert_ne!(refs, 0);
                Err(RetypeError::RefsExist(refs))
            }
            Err((other_state, _refs)) => Err(RetypeError::InvalidFromState(other_state)),
        }
    }

    pub fn try_into_untyped(self) -> Result<RawFrame, RetypeError> {
        if self.try_into_untyped_from(State::User).is_ok() {
            return Ok(self);
        }
        self.try_into_untyped_from(State::Kernel)?;
        Ok(self)
    }
}

impl UserFrame {
    fn entry(&self) -> &'static RetypeEntry {
        // SAFETY: Entry must exist if a KernelFrame exists.
        unsafe { self.0.retype_entry().unwrap_unchecked() }
    }

    pub fn frame(&self) -> RawFrame {
        self.0
    }

    pub fn into_raw(self) -> RawFrame {
        ManuallyDrop::new(self).0
    }

    pub fn try_clone(&self) -> Option<Self> {
        self.0.retype_entry().unwrap().increment().ok()?;
        Some(Self(self.frame()))
    }

    pub fn drop(self) -> u16 {
        self.entry().decrement().unwrap()
    }
}

#[repr(transparent)]
pub struct KernelFrame(RawFrame);

impl KernelFrame {
    fn entry(&self) -> &'static RetypeEntry {
        // SAFETY: Entry must exist if a KernelFrame exists.
        unsafe { self.0.retype_entry().unwrap_unchecked() }
    }

    pub fn frame(&self) -> RawFrame {
        self.0
    }

    pub fn into_raw(self) -> RawFrame {
        ManuallyDrop::new(self).0
    }

    /// Builds back a kernel frame from the raw frame
    ///
    /// # Safety
    ///
    /// The frame must have been created with `into_raw`.
    pub unsafe fn from_raw(frame: RawFrame) -> Self {
        Self(frame)
    }

    pub fn try_clone(&self) -> Option<Self> {
        self.0.retype_entry().unwrap().increment().ok()?;
        Some(Self(self.frame()))
    }

    pub fn drop(self) -> u16 {
        self.entry().decrement().unwrap()
    }
}

impl Drop for KernelFrame {
    fn drop(&mut self) {
        self.entry().decrement().unwrap();
    }
}

impl Drop for UserFrame {
    fn drop(&mut self) {
        self.entry().decrement().unwrap();
    }
}

#[repr(transparent)]
#[derive(Debug)]
struct RetypeEntry(AtomicU16);

impl RetypeEntry {
    const STATE_BITS: u16 = 3;
    const COUNTER_BITS: u16 = 16 - Self::STATE_BITS;
    pub const MAX_REF_COUNT: u16 = 1 << Self::COUNTER_BITS - 1;
    const fn value_for(state: State, counter: u16) -> u16 {
        assert!(counter <= Self::MAX_REF_COUNT);
        (state as u8 as u16) << Self::COUNTER_BITS + counter
    }

    const fn value_into(value: u16) -> (State, u16) {
        let counter = value & ((1 << Self::COUNTER_BITS) - 1);
        // SAFETY: All possible variants are covered for
        let state = unsafe { core::mem::transmute((value >> Self::COUNTER_BITS) as u8) };
        (state, counter)
    }

    pub const fn unavailable() -> Self {
        Self(AtomicU16::new(Self::value_for(State::Unavailable, 0)))
    }

    pub const fn untyped() -> Self {
        Self(AtomicU16::new(Self::value_for(State::Untyped, 0)))
    }

    pub fn increment(&self) -> Result<u16, MaxRefs> {
        self.0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                let (_, counter) = Self::value_into(value);
                if counter == Self::MAX_REF_COUNT {
                    None
                } else {
                    Some(value + 1)
                }
            })
            .map(|entry| Self::value_into(entry).1)
            .map_err(|_| MaxRefs)
    }

    pub fn decrement(&self) -> Result<u16, NoRefs> {
        self.0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                let (_, counter) = Self::value_into(value);
                if counter == 0 {
                    None
                } else {
                    Some(value - 1)
                }
            })
            .map(|entry| Self::value_into(entry).1)
            .map_err(|_| NoRefs)
    }

    pub fn get_as_and_increment(&self, wants: State) -> Result<(), (State, u16)> {
        self.0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                let (state, count) = Self::value_into(value);
                if wants == state && count < Self::MAX_REF_COUNT {
                    Some(Self::value_for(state, count + 1))
                } else {
                    None
                }
            })
            .map(|_| ())
            .map_err(Self::value_into)
    }

    pub fn retype(
        &self,
        from_state: State,
        to_state: State,
        from_counter: u16,
        to_counter: u16,
    ) -> Result<(), (State, u16)> {
        let from = Self::value_for(from_state, from_counter);
        let to = Self::value_for(to_state, to_counter);
        self.0
            .compare_exchange(from, to, Ordering::Relaxed, Ordering::Relaxed)
            .map_err(|current| Self::value_into(current))?;

        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[repr(u8)]
pub enum State {
    Unavailable = 0,
    Untyped = 1,
    User = 2,
    Kernel = 3,
}
