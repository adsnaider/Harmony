use crate::arch::paging::{PhysAddr, RawFrame, FRAME_SIZE};
use crate::retyping::{AsTypeError, KernelFrame, RetypeError, UserFrame};

pub struct BumpAllocator {
    index: u64,
}

impl Default for BumpAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl BumpAllocator {
    pub fn new() -> Self {
        Self { index: 0 }
    }

    pub fn alloc_user_frame(&mut self) -> Option<UserFrame> {
        loop {
            let frame = self.next_available();
            self.index += 1;
            log::trace!("Trying to allocate user frame: {frame:?}");
            match frame.try_into_user() {
                Ok(frame) => return Some(frame),
                Err(RetypeError::OutOfBounds) => return None,
                Err(e) => log::trace!("Err: {e:?}"),
            }
        }
    }

    pub fn alloc_untyped_frame(&mut self) -> Option<RawFrame> {
        loop {
            let frame = self.next_available();
            self.index += 1;
            log::trace!("Trying to allocate untyped frame: {frame:?}");
            match frame.try_as_untyped() {
                Ok(frame) => return Some(frame),
                Err(AsTypeError::OutOfBounds) => return None,
                Err(e) => log::trace!("Err: {e:?}"),
            }
        }
    }

    pub fn alloc_kernel_frame(&mut self) -> Option<KernelFrame> {
        loop {
            let frame = self.next_available();
            self.index += 1;
            log::trace!("Trying to allocate kernel frame: {frame:?}");
            match frame.try_into_kernel() {
                Ok(frame) => return Some(frame),
                Err(RetypeError::OutOfBounds) => return None,
                Err(e) => log::trace!("Err: {e:?}"),
            }
        }
    }

    pub fn next_available(&self) -> RawFrame {
        RawFrame::from_start_address(PhysAddr::new(FRAME_SIZE * self.index))
    }
}
