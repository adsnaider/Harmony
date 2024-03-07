use core::marker::PhantomData;
use core::ops::Deref;

use super::frames::RawFrame;
use super::retyping::{KernelFrame, UntypedFrame};

#[derive(Debug, Clone)]
pub struct KPtr<T> {
    frame: KernelFrame<'static>,
    _value: PhantomData<T>,
}

impl<T> KPtr<T> {
    /// Creates a new kernel pointer to a `T`.
    ///
    /// # Safety
    ///
    /// The frame must either be unused or contain valid data of
    pub fn new(frame: UntypedFrame<'static>, value: T) -> Self {
        const {
            assert!(core::mem::size_of::<T>() == RawFrame::size());
            assert!(core::mem::align_of::<T>() == RawFrame::align());
        }
        let frame = frame.into_kernel();
        let ptr: *mut T = frame.raw().as_ptr_mut();
        unsafe { ptr.write(value) }
        Self {
            frame,
            _value: PhantomData,
        }
    }

    pub fn set(&self, to: &KPtr<T>) {
        todo!();
    }
}

impl<T> KPtr<T> {
    pub fn as_ptr(&self) -> *const T {
        self.frame.raw().as_ptr()
    }

    pub fn as_ptr_mut(&self) -> *mut T {
        self.frame.raw().as_ptr_mut()
    }
}

impl<T> AsRef<T> for KPtr<T> {
    fn as_ref(&self) -> &T {
        &*self
    }
}

impl<T> Deref for KPtr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.as_ptr() }
    }
}

unsafe impl<T> Send for KPtr<T> where T: Send + Sync {}
unsafe impl<T> Sync for KPtr<T> where T: Send + Sync {}
