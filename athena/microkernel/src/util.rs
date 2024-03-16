use crate::arch::paging::RawFrame;
use crate::retyping::{RetypeError, UntypedFrame};

pub struct FrameBumpAllocator {
    index: usize,
}

impl FrameBumpAllocator {
    pub fn new() -> Self {
        Self { index: 0 }
    }

    pub fn alloc_frame(&mut self) -> Option<UntypedFrame<'static>> {
        loop {
            let frame = RawFrame::from_index(self.index);
            match UntypedFrame::from_raw(frame) {
                Ok(frame) => return Some(frame),
                Err(RetypeError::OutOfBounds) => return None,
                Err(_) => {}
            }
            self.index += 1;
        }
    }
}
