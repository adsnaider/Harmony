use bitvec::slice::BitSlice;
use kapi::{
    ops::memory::RetypeKind,
    raw::CapError,
    userspace::{
        paging::{
            addr::{Frame, Page},
            Addrspace, RetypeState, RetypeTable,
        },
        structures::Retype,
    },
};

pub struct BitmapAllocator<'a> {
    unused_frames: &'a mut BitSlice,
}

impl BitmapAllocator<'static> {
    pub fn bootstrap(memory_map: RetypeTable<'static>, heap_start: usize) -> Self {
        let bits_required = memory_map.frame_count();
        let bytes_required = bits_required.div_ceil(u8::BITS as _);
        let frames_required = bytes_required.div_ceil(Page::size());

        let slice_length = bits_required.div_ceil(usize::BITS as _);
        todo!();
    }
}

impl<'a> BitmapAllocator<'a> {
    pub fn new(store: &'a mut [usize], memory_map: RetypeTable<'_>) -> Self {
        let unused_frames = BitSlice::from_slice_mut(store);
        let mut initializer = memory_map
            .iter()
            .map(|(state, _frame)| state.state == RetypeState::Untyped)
            .chain(core::iter::repeat(false));
        unused_frames.fill_with(|_| initializer.next().unwrap());
        Self { unused_frames }
    }

    pub fn alloc(&mut self) -> Option<Frame> {
        let index = self.unused_frames.first_one()?;
        self.unused_frames.set(index, false);
        Some(Frame::from_index(index as u64).expect("Index is within range of bitmap"))
    }

    pub unsafe fn dealloc(&mut self, frame: Frame) -> Result<(), OutOfRange> {
        let index = frame.index() as usize;
        if index >= self.unused_frames.len() {
            return Err(OutOfRange);
        }
        debug_assert!(!self.unused_frames[index]);
        self.unused_frames.set(index, true);
        Ok(())
    }
}

#[derive(Debug)]
pub struct OutOfRange;
