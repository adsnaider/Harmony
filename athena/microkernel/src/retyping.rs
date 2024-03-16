//! The memory retyping table and objects

use core::mem::{ManuallyDrop, MaybeUninit};
use core::sync::atomic::{AtomicU16, AtomicU8, Ordering};

use limine::memory_map::{Entry as MMapEntry, EntryType};
use once_cell::sync::OnceCell;

use crate::arch::paging::{RawFrame, PAGE_SIZE};
use crate::PMO;

pub struct RetypeTable<'a> {
    table: &'a [Entry],
}

static RETYPE_TBL: OnceCell<RetypeTable<'static>> = OnceCell::new();

pub fn init(memory_map: &'static mut [&'static mut MMapEntry]) {
    RETYPE_TBL
        .set(RetypeTable::from_memory_map(memory_map).unwrap())
        .unwrap_or_else(|_| panic!("Double initialization"));
}

#[derive(Debug)]
pub enum RetypeInitError {
    NotEnoughFrames,
}

#[derive(Debug)]
pub enum RetypeError {
    NotUntyped(State),
    OutOfBounds,
}

#[derive(Debug)]
pub enum EntryError {
    OutOfBounds,
}

impl<'a> RetypeTable<'a> {
    /// Constructs the retype table from the bootstrap frame allocator.
    pub fn from_memory_map(
        memory_map: &'a mut [&'a mut MMapEntry],
    ) -> Result<RetypeTable<'static>, RetypeInitError> {
        // TODO: Do this better by allocating multiple frames as necessary and remapping them.
        let nframes = memory_map.iter().fold(0, |last, region| {
            usize::max(last, (region.base + region.length) as usize / PAGE_SIZE)
        });

        let bytes_required = (nframes - 1) / core::mem::size_of::<StateValue>() + 1;
        let frames_required = (bytes_required - 1) / 4096 + 1;

        let stolen_region = memory_map
            .iter_mut()
            .filter(|reg| matches!(reg.entry_type, EntryType::USABLE))
            .find(|reg| reg.length >= bytes_required as u64)
            .ok_or(RetypeInitError::NotEnoughFrames)?;

        assert!(stolen_region.base as usize % PAGE_SIZE == 0);
        assert!(stolen_region.length as usize % PAGE_SIZE == 0);
        let store = (*PMO + stolen_region.base as usize) as *mut MaybeUninit<Entry>;
        stolen_region.base += frames_required as u64 * PAGE_SIZE as u64;
        stolen_region.length -= frames_required as u64 * PAGE_SIZE as u64;

        assert!(nframes * core::mem::size_of::<StateValue>() <= frames_required * PAGE_SIZE);
        assert!(core::mem::align_of::<StateValue>() % store as usize == 0);

        let store = unsafe { core::slice::from_raw_parts_mut(store, nframes) };

        for rc in store.iter_mut() {
            rc.write(Entry {
                state: StateValue::unavailable(),
                counter: AtomicU16::new(0),
            });
        }
        let store: &'static mut [Entry] = unsafe { core::mem::transmute(store) };
        for region in memory_map
            .iter()
            .filter(|reg| matches!(reg.entry_type, EntryType::USABLE))
        {
            let fstart = region.base as usize / PAGE_SIZE;
            let fend = (region.base + region.length) as usize / 4096;
            for fnum in fstart..fend {
                store[fnum].state = StateValue::untyped();
            }
        }
        Ok(RetypeTable { table: store })
    }

    fn raw_entry(&self, frame: RawFrame) -> Result<&'a Entry, EntryError> {
        self.table.get(frame.index()).ok_or(EntryError::OutOfBounds)
    }

    pub fn entry(&self, frame: RawFrame) -> Result<UntypedFrame<'a>, RetypeError> {
        let entry = self
            .table
            .get(frame.index())
            .ok_or(RetypeError::OutOfBounds)?;
        // TODO: Atomic ordering correctness?
        let result = entry.state.compare_exchange(
            StateValue::untyped(),
            StateValue::retyping(),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        match result {
            Ok(()) => Ok(UntypedFrame { frame, entry }),
            Err(current) => Err(RetypeError::NotUntyped(current.into_state())),
        }
    }
}
impl RawFrame {
    pub fn into_untyped(self) -> Result<UntypedFrame<'static>, RetypeError> {
        RETYPE_TBL.get().unwrap().entry(self)
    }

    pub unsafe fn as_kernel_frame(&self) -> KernelFrame<'static> {
        let entry = RETYPE_TBL
            .get()
            .unwrap()
            .table
            .get(self.index())
            .expect("Frame out of bounds");
        entry.counter.fetch_add(1, Ordering::Release);
        KernelFrame {
            entry,
            frame: self.clone(),
        }
    }

    pub unsafe fn as_user_frame(&self) -> UserFrame<'static> {
        let entry = RETYPE_TBL
            .get()
            .unwrap()
            .table
            .get(self.index())
            .expect("Frame out of bounds");
        entry.counter.fetch_add(1, Ordering::Release);
        UserFrame {
            entry,
            frame: self.clone(),
        }
    }
}

#[derive(Debug)]
pub struct UntypedFrame<'a> {
    frame: RawFrame,
    entry: &'a Entry,
}

#[derive(Debug)]
pub struct KernelFrame<'a> {
    frame: RawFrame,
    entry: &'a Entry,
}

#[derive(Debug)]
pub struct UserFrame<'a> {
    frame: RawFrame,
    entry: &'a Entry,
}

impl<'a> UntypedFrame<'a> {
    pub fn into_kernel(self) -> KernelFrame<'a> {
        self.entry.counter.store(1, Ordering::Release);
        self.entry
            .state
            .store(StateValue::kernel(), Ordering::Release);
        KernelFrame {
            frame: self.frame,
            entry: &self.entry,
        }
    }

    pub fn into_user(self) -> UserFrame<'a> {
        self.entry.counter.store(1, Ordering::Release);
        self.entry
            .state
            .store(StateValue::user(), Ordering::Release);
        UserFrame {
            frame: self.frame,
            entry: &self.entry,
        }
    }

    pub fn as_raw(&self) -> RawFrame {
        self.frame
    }
}

impl UntypedFrame<'static> {
    pub fn into_raw(self) -> RawFrame {
        // Note, we don't drop this, so the underlying type remains
        let this = ManuallyDrop::new(self);
        this.frame
    }

    pub fn from_raw(raw: RawFrame) -> Result<Self, RetypeError> {
        RETYPE_TBL.get().unwrap().entry(raw)
    }
}

#[derive(Debug)]
pub enum ToUntypedError {
    ReferencesExist,
}

impl<'a> KernelFrame<'a> {
    pub fn try_into_untyped(self) -> Result<UntypedFrame<'a>, ToUntypedError> {
        todo!();
    }

    pub fn raw(&self) -> RawFrame {
        self.frame
    }

    pub fn into_raw(self) -> RawFrame {
        // Note, we don't drop this, so the underlying type remains
        let this = ManuallyDrop::new(self);
        this.frame
    }

    pub unsafe fn inc(&self) -> u16 {
        let counter = self.entry.counter.fetch_add(1, Ordering::AcqRel);
        assert!(counter < MAX_REFCOUNT);
        counter
    }

    pub unsafe fn dec(&self) -> u16 {
        let counter = self.entry.counter.fetch_sub(1, Ordering::AcqRel);
        assert!(counter > 0);
        counter
    }
}

impl KernelFrame<'static> {
    pub unsafe fn from_raw(raw: RawFrame) -> Result<Self, EntryError> {
        let entry = RETYPE_TBL.get().unwrap().raw_entry(raw)?;
        Ok(Self { frame: raw, entry })
    }
}

impl<'a> UserFrame<'a> {
    pub fn try_into_untyped(self) -> Result<UntypedFrame<'a>, ToUntypedError> {
        todo!();
    }
    pub fn raw(&self) -> RawFrame {
        self.frame
    }

    pub fn into_raw(self) -> RawFrame {
        // Note, we don't drop this, so the underlying type remains
        let this = ManuallyDrop::new(self);
        this.frame
    }

    pub unsafe fn inc(&self) -> u16 {
        let counter = self.entry.counter.fetch_add(1, Ordering::AcqRel);
        assert!(counter < MAX_REFCOUNT);
        counter
    }

    pub unsafe fn dec(&mut self) -> u16 {
        let counter = self.entry.counter.fetch_sub(1, Ordering::AcqRel);
        assert!(counter > 0);
        counter
    }
}

impl UserFrame<'static> {
    pub unsafe fn from_raw(raw: RawFrame) -> Result<Self, EntryError> {
        let entry = RETYPE_TBL.get().unwrap().raw_entry(raw)?;
        Ok(Self { frame: raw, entry })
    }
}

const MAX_REFCOUNT: u16 = i16::MAX as u16;

#[repr(transparent)]
#[derive(Debug)]
struct StateValue(AtomicU8);

#[derive(Debug)]
pub enum State {
    Unavailable = 0,
    Retyping = 1,
    Untyped = 2,
    User = 3,
    Kernel = 4,
}

impl StateValue {
    pub fn unavailable() -> Self {
        Self(AtomicU8::new(0))
    }
    pub fn retyping() -> Self {
        Self(AtomicU8::new(1))
    }
    pub fn untyped() -> Self {
        Self(AtomicU8::new(2))
    }
    pub fn user() -> Self {
        Self(AtomicU8::new(3))
    }
    pub fn kernel() -> Self {
        Self(AtomicU8::new(4))
    }

    fn new(value: u8) -> Self {
        assert!(value <= 4);
        Self(value.into())
    }

    pub fn into_state(self) -> State {
        match self.0.into_inner() {
            0 => State::Unavailable,
            1 => State::Retyping,
            2 => State::Untyped,
            3 => State::User,
            4 => State::Kernel,
            other => panic!("Unknown memory state: {other}"),
        }
    }

    pub fn compare_exchange(
        &self,
        current: StateValue,
        new: StateValue,
        success: Ordering,
        failure: Ordering,
    ) -> Result<(), StateValue> {
        let current = current.0.into_inner();
        let new = new.0.into_inner();
        match self.0.compare_exchange(current, new, success, failure) {
            Ok(_) => Ok(()),
            Err(current) => Err(Self::new(current)),
        }
    }

    pub fn store(&self, value: StateValue, order: Ordering) {
        self.0.store(value.0.into_inner(), order);
    }
}

#[derive(Debug)]
struct Entry {
    state: StateValue,
    counter: AtomicU16,
}
