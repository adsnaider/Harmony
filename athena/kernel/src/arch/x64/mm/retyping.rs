//! Memory retyping implementation

use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU16, AtomicU8, Ordering};

use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use thiserror::Error;

use super::frames::RawFrame;

pub struct RetypeTable<'a> {
    table: &'a [Entry],
}

#[derive(Error, Debug)]
pub enum RetypeInitError {
    #[error("Not enough contiguous frames in the system")]
    NotEnoughFrames,
}

#[derive(Error, Debug)]
pub enum RetypeError {
    #[error("The frame is not untyped")]
    NotUntyped(State),
    #[error("The frame extends past the end of available memory")]
    OutOfBounds,
}

impl RetypeTable<'_> {
    pub unsafe fn from_mmap(
        memory_map: &mut MemoryRegions,
    ) -> Result<RetypeTable<'static>, RetypeInitError> {
        let nframes = memory_map.iter().fold(0, |last, region| {
            usize::max(last, region.end as usize / 4096)
        });

        let bytes_required = (nframes - 1) / core::mem::size_of::<StateValue>() + 1;
        let frames_required = (bytes_required - 1) / 4096 + 1;

        let stolen_region = memory_map
            .iter_mut()
            .filter(|reg| matches!(reg.kind, MemoryRegionKind::Usable))
            .find(|reg| reg.end - reg.start >= bytes_required as u64)
            .ok_or(RetypeInitError::NotEnoughFrames)?;

        assert!(stolen_region.start % 4096 == 0);
        assert!(stolen_region.end % 4096 == 0);
        let store = stolen_region.start as *mut MaybeUninit<Entry>;
        stolen_region.start += frames_required as u64 * 4096;
        assert!(stolen_region.start <= stolen_region.end);

        assert!(nframes * core::mem::size_of::<StateValue>() <= frames_required * 4096);
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
            .filter(|reg| matches!(reg.kind, MemoryRegionKind::Usable))
        {
            let fstart = region.start / 4096;
            let fend = region.end / 4096;
            for fnum in fstart..fend {
                store[fnum as usize].state = StateValue::untyped();
            }
        }
        Ok(RetypeTable { table: store })
    }

    pub fn entry(&self, frame: RawFrame) -> Result<UntypedFrame, RetypeError> {
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

#[derive(Debug)]
pub struct RetypeEntry {
    frame: RawFrame,
    ty: StateValue,
}

impl RetypeEntry {}

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
    pub fn raw(&self) -> RawFrame {
        self.frame
    }
}

#[derive(Debug, Error)]
pub enum ToUntypedError {
    #[error("References still exist to the frame")]
    ReferencesExist,
}

impl<'a> KernelFrame<'a> {
    pub fn try_into_untyped(self) -> Result<UntypedFrame<'a>, ToUntypedError> {
        todo!();
    }

    pub fn raw(&self) -> RawFrame {
        self.frame
    }
}

impl<'a> UserFrame<'a> {
    pub fn try_into_untyped(self) -> Result<UntypedFrame<'a>, ToUntypedError> {
        todo!();
    }
    pub fn raw(&self) -> RawFrame {
        self.frame
    }
}

const MAX_REFCOUNT: u16 = i16::MAX as u16;

impl Clone for KernelFrame<'_> {
    fn clone(&self) -> Self {
        let counter = self.entry.counter.fetch_add(1, Ordering::AcqRel);
        assert!(counter < MAX_REFCOUNT);
        KernelFrame {
            frame: self.frame,
            entry: self.entry,
        }
    }
}

impl Drop for KernelFrame<'_> {
    fn drop(&mut self) {
        if self.entry.counter.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.entry
                .state
                .store(StateValue::untyped(), Ordering::AcqRel);
        }
    }
}

impl Clone for UserFrame<'_> {
    fn clone(&self) -> Self {
        let counter = self.entry.counter.fetch_add(1, Ordering::AcqRel);
        assert!(counter < MAX_REFCOUNT);
        UserFrame {
            frame: self.frame,
            entry: self.entry,
        }
    }
}

impl Drop for UserFrame<'_> {
    fn drop(&mut self) {
        if self.entry.counter.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.entry
                .state
                .store(StateValue::untyped(), Ordering::AcqRel);
        }
    }
}

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
