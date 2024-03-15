//! Kernel datastructures that fit into a page.

use core::ops::Deref;
use core::ptr::NonNull;

use crate::arch::paging::{RawFrame, PAGE_SIZE};
use crate::retyping::{KernelFrame, UntypedFrame};

/// The generic kernel resource.
///
/// Each resource must be page-aligned and must fit within a page.
///
/// We unfortunately use a union instead of an enum because we need
/// to maintain the size and alignment of the kernel datastructures.
///
/// What we really want is a generic "fat" ResourcePtr that holds the pointer
/// to a resource and the tag and safely provides access to the internal data.
#[repr(C)]
pub union Resource {}

/// Tag to describe a resource.
#[repr(u8)]
#[derive(Debug, Clone)]
pub enum ResourceType {}

/// A "kernel" pointer to any page-aligned resource.
///
/// A kernel pointer is a pointer type that may only point to a kernel object
/// that takes up an entire page. This is because kernel pointers use the
/// memroy retyping capabilities which use reference counts on an entire
/// page.
#[repr(transparent)]
pub struct KPtr<T> {
    inner: NonNull<T>,
}

impl<T> KPtr<T> {
    const _SIZE_AND_ALIGN: () = {
        assert!(core::mem::size_of::<T>() == PAGE_SIZE);
        assert!(core::mem::align_of::<T>() == PAGE_SIZE);
    };

    pub fn new(frame: UntypedFrame<'static>, value: T) -> Self {
        let frame = frame.into_kernel().into_raw();
        let ptr: NonNull<T> = NonNull::new(frame.as_ptr_mut()).unwrap();
        unsafe {
            ptr.as_ptr().write(value);
        }
        Self { inner: ptr }
    }

    pub fn frame(&self) -> KernelFrame<'static> {
        // SAFETY: Pointer was created from the frame.
        let frame = unsafe { RawFrame::from_ptr(self.inner.as_ptr()) };
        // SAFETY: Frame must be of type kernel frame since it comes from a kernel pointer.
        unsafe { frame.as_kernel_frame() }
    }
}

impl<T> Deref for KPtr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.inner.as_ref() }
    }
}

impl<T> Clone for KPtr<T> {
    fn clone(&self) -> Self {
        let frame = self.frame();
        unsafe {
            frame.inc();
        }
        Self { inner: self.inner }
    }
}

impl<T> Drop for KPtr<T> {
    fn drop(&mut self) {
        let frame = self.frame();
        let count = unsafe { frame.dec() };
        if count == 1 {
            // last one turns off the lights
            unsafe { drop(self.inner.as_ptr().read()) }
        }
    }
}

const _RESOURCE_SIZE_AND_ALIGN: () = {
    assert!(core::mem::size_of::<Resource>() == PAGE_SIZE);
    assert!(core::mem::align_of::<Resource>() == PAGE_SIZE);
};
