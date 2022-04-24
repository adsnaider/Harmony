//! Physical frame allocation.

pub mod bitmap_frame_allocator;

use crate::structures::{Frame, PageSize};

/// Frame allocation error.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FrameAllocError {
    /// No frames of the requested size are availabe.
    OutOfFrames,
}

/// Frame deallocation errror.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FrameDeallocError {
    /// Tried to deallocate a frame that wasn't rpeviously allocated with the same allocator.
    FrameNotAllocated,
}

/// A frame allocator manages frames in the system.
///
/// # Safety
///
/// * The allocator can't return an already allocated frame for another allocation.
/// * The returned frame must have been available in the system for use.
pub unsafe trait FrameAllocator<S: PageSize> {
    /// Allocates a frame of the specified size in the system.
    ///
    /// # Returns
    ///
    /// A well-built frame that can be used for any of your memory needs. The `frame` will be of
    /// the requested size. However, if there are no frames of the provided size, then it returns
    /// an error.
    fn allocate_frame(&mut self) -> Result<Frame<S>, FrameAllocError>;
    /// Deallocates a previously allocated frame.
    ///
    /// # Safety
    ///
    /// No references into the frame can exist after deallocation. The memory will be reclaimed and
    /// reused by the system.
    unsafe fn deallocate_frame(&mut self, frame: Frame<S>) -> Result<(), FrameDeallocError>;
}

/// Request frame error.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RequestFrameError {
    /// Can't allocate the specific frame because the frame is currently in use.
    FrameInUse,
}

/// Frame allocator that also permits requests for specific physical frames.
///
/// # Safety
///
/// Same as [`FrameAllocator`] where [`ExactFrameAllocator::request_frame`] must not allocate
/// frames in use.
pub unsafe trait ExactFrameAllocator<S: PageSize>: FrameAllocator<S> {
    /// Request for a specific frame allocation.
    ///
    /// # Returns
    ///
    /// `Ok(())`, if the request is successfull. In this case, it's reasonable for the caller to use
    /// the memory in the frame.
    ///
    /// `Err(...)` if the request couldn't be fulfilled. It's unsafe to use the memory in the frame
    /// in this case.
    fn request_frame(&mut self, frame: Frame<S>) -> Result<(), RequestFrameError>;
}
