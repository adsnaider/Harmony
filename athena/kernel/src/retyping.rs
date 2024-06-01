use core::mem::{ManuallyDrop, MaybeUninit};
use core::sync::atomic::{AtomicU16, Ordering};

use limine::memory_map::EntryType;
use sync::cell::AtomicOnceCell;

use crate::arch::paging::page_table::AnyPageTable;
use crate::arch::paging::{RawFrame, FRAME_SIZE, PAGE_SIZE};
use crate::retyping::bump_alloc::BumpAllocator;
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
        for entry in memory_map.iter() {
            assert!(entry.base % FRAME_SIZE == 0);
            assert!(entry.length % FRAME_SIZE == 0);
            let start_idx = (entry.base / FRAME_SIZE) as usize;
            let count = (entry.length / FRAME_SIZE) as usize;
            for i in start_idx..(start_idx + count) {
                let retype_entry = match entry.entry_type {
                    EntryType::USABLE => RetypeEntry::untyped(),
                    EntryType::BOOTLOADER_RECLAIMABLE | EntryType::KERNEL_AND_MODULES => {
                        RetypeEntry::kernel(1)
                    }
                    _ => RetypeEntry::unavailable(),
                };
                if i == 0x7E34000 / PAGE_SIZE {
                    log::info!("Retyping Cr3 as {retype_entry:?}");
                }
                retype_map[i] = retype_entry;
            }
        }
        Some(Self { retype_map })
    }

    pub fn init(self) -> Result<(), sync::cell::OnceError> {
        RETYPE_TABLE.set(self)?;
        // Set the current l4_table as a kernel frame.
        let l4_table = AnyPageTable::current_raw();
        l4_table.try_as_kernel().unwrap();
        Ok(())
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
    pub fn memory_size() -> usize {
        let nframes = RETYPE_TABLE.get().unwrap().retype_map.len();
        nframes * FRAME_SIZE as usize
    }

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
        log::trace!("Turning {self:?} as user frame");
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
        log::trace!("Turning {self:?} as kernel frame");
        self.retype_entry()?
            .get_as_and_increment(State::Kernel)
            .map_err(|(state, value)| {
                if !matches!(state, State::Kernel) {
                    AsTypeError::NotExpectedState(state)
                } else {
                    debug_assert!(value == RetypeEntry::MAX_REF_COUNT);
                    AsTypeError::MaxRefs
                }
            })?;
        Ok(KernelFrame(self))
    }

    pub fn try_as_untyped(self) -> Result<RawFrame, AsTypeError> {
        log::trace!("Trying to get {self:?} as untyped");
        let (state, _count) = self.retype_entry()?.get();
        if !matches!(state, State::Untyped) {
            return Err(AsTypeError::NotExpectedState(state));
        }
        Ok(self)
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
#[derive(Debug)]
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
        log::trace!("Dropping {self:?}");
        self.entry().decrement().unwrap();
    }
}

impl Drop for UserFrame {
    fn drop(&mut self) {
        log::trace!("Dropping {self:?}");
        self.entry().decrement().unwrap();
    }
}

#[repr(transparent)]
#[derive(Debug)]
struct RetypeEntry(AtomicU16);

#[derive(Debug)]
struct Invalid;
impl State {
    const fn try_from(value: u8) -> Result<Self, Invalid> {
        match value {
            0 => Ok(State::Unavailable),
            1 => Ok(State::Untyped),
            2 => Ok(State::User),
            3 => Ok(State::Kernel),
            _ => Err(Invalid),
        }
    }
}

#[allow(unused)]
impl RetypeEntry {
    const STATE_BITS: u16 = 2;
    const COUNTER_BITS: u16 = 16 - Self::STATE_BITS;
    pub const MAX_REF_COUNT: u16 = (1 << Self::COUNTER_BITS) - 1;

    fn value_for(state: State, counter: u16) -> u16 {
        assert!(counter <= Self::MAX_REF_COUNT);

        ((state as u8 as u16) << Self::COUNTER_BITS) + counter % Self::MAX_REF_COUNT
    }

    const fn value_into(value: u16) -> (State, u16) {
        let counter = value & ((1 << Self::COUNTER_BITS) - 1);
        let state = match State::try_from((value >> Self::COUNTER_BITS) as u8) {
            Ok(state) => state,
            Err(_e) => panic!("Invalid retype state"),
        };
        (state, counter)
    }

    pub fn unavailable() -> Self {
        Self(AtomicU16::new(Self::value_for(State::Unavailable, 0)))
    }

    pub fn untyped() -> Self {
        Self(AtomicU16::new(Self::value_for(State::Untyped, 0)))
    }

    pub fn kernel(ref_count: u16) -> Self {
        Self(AtomicU16::new(Self::value_for(State::Kernel, ref_count)))
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

    pub fn get(&self) -> (State, u16) {
        Self::value_into(self.0.load(Ordering::Relaxed))
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

    pub fn set(&mut self, state: State, value: u16) {
        let to = Self::value_for(state, value);
        *self.0.get_mut() = to;
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

mod bump_alloc {
    use limine::memory_map::{Entry, EntryType};

    use crate::arch::paging::{PhysAddr, RawFrame, FRAME_SIZE};
    use crate::MemoryMap;

    pub struct BumpAllocator {
        memory_map: MemoryMap,
        index: usize,
    }

    #[allow(unused)]
    impl BumpAllocator {
        pub fn new(memory_map: MemoryMap) -> Self {
            Self {
                memory_map,
                index: 0,
            }
        }

        pub fn alloc_frame(&mut self) -> Option<RawFrame> {
            let frame = loop {
                let entry = self.memory_map.get_mut(self.index)?;
                assert!(entry.length % FRAME_SIZE == 0);
                if entry.entry_type == EntryType::USABLE && entry.length > 0 {
                    let start_address = entry.base;
                    entry.base += FRAME_SIZE;
                    entry.length -= FRAME_SIZE;
                    let start_address = PhysAddr::new(start_address);
                    break RawFrame::from_start_address(start_address);
                }
                self.index += 1;
            };
            Some(frame)
        }

        pub fn alloc_frames(&mut self, count: usize) -> Option<PhysAddr> {
            let requested_length = count as u64 * FRAME_SIZE;
            let start_address = loop {
                let entry = self.memory_map.get_mut(self.index)?;
                assert!(entry.length % FRAME_SIZE == 0);
                if entry.entry_type == EntryType::USABLE && entry.length >= requested_length {
                    let start_address = entry.base;
                    entry.base += requested_length;
                    entry.length -= requested_length;
                    break PhysAddr::new(start_address);
                }
                self.index += 1;
            };
            Some(start_address)
        }

        pub fn into_memory_map(self) -> &'static mut [&'static mut Entry] {
            self.memory_map
        }

        pub fn memory_map(&mut self) -> &mut [&'static mut Entry] {
            self.memory_map
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test_case]
        fn allocation_test() {
            static mut TEST_MAP: [&mut Entry; 4] = [
                &mut Entry {
                    base: 0,
                    length: FRAME_SIZE * 2,
                    entry_type: EntryType::USABLE,
                },
                &mut Entry {
                    base: FRAME_SIZE * 4,
                    length: FRAME_SIZE,
                    entry_type: EntryType::USABLE,
                },
                &mut Entry {
                    base: FRAME_SIZE * 5,
                    length: FRAME_SIZE,
                    entry_type: EntryType::RESERVED,
                },
                &mut Entry {
                    base: FRAME_SIZE * 6,
                    length: FRAME_SIZE,
                    entry_type: EntryType::USABLE,
                },
            ];

            // SAFETY: Mutable access is unique.
            #[allow(static_mut_refs)]
            let mut allocator = BumpAllocator::new(unsafe { &mut TEST_MAP });
            for expected_frame in [0, FRAME_SIZE, FRAME_SIZE * 4, FRAME_SIZE * 6] {
                let expected_frame = RawFrame::from_start_address(PhysAddr::new(expected_frame));
                let frame = allocator.alloc_frame().unwrap();
                assert_eq!(frame, expected_frame);
            }

            assert!(allocator.alloc_frame().is_none())
        }

        #[test_case]
        fn multi_allocation_test() {
            static mut TEST_MAP: [&mut Entry; 4] = [
                &mut Entry {
                    base: 0,
                    length: FRAME_SIZE * 2,
                    entry_type: EntryType::USABLE,
                },
                &mut Entry {
                    base: FRAME_SIZE * 4,
                    length: FRAME_SIZE,
                    entry_type: EntryType::USABLE,
                },
                &mut Entry {
                    base: FRAME_SIZE * 5,
                    length: FRAME_SIZE,
                    entry_type: EntryType::RESERVED,
                },
                &mut Entry {
                    base: FRAME_SIZE * 6,
                    length: FRAME_SIZE * 2,
                    entry_type: EntryType::USABLE,
                },
            ];

            // SAFETY: Mutable access is unique.
            #[allow(static_mut_refs)]
            let mut allocator = BumpAllocator::new(unsafe { &mut TEST_MAP });
            for expected_start in [0, FRAME_SIZE * 6]
                .into_iter()
                .map(|addr| PhysAddr::new(addr))
            {
                let start = allocator.alloc_frames(2).unwrap();
                assert_eq!(start, expected_start);
                assert!(start.as_u64() % FRAME_SIZE == 0);
            }

            assert!(allocator.alloc_frame().is_none())
        }
    }
}
