use core::marker::PhantomData;

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, TryFromBytes, Unalign};

#[repr(C)]
#[derive(TryFromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct CSlice<'a, T> {
    ptr: OrphanPtr<T>,
    length: usize,
    _cont: PhantomData<&'a [T]>,
}

/// A raw pointer that has no provenance information, e.g. used for syscalls
#[repr(transparent)]
#[derive(IntoBytes, KnownLayout, Immutable, FromBytes)]
pub struct OrphanPtr<T> {
    addr: usize,
    _phantom: PhantomData<*const T>,
}

impl<T> Copy for CSlice<'static, T> {}
impl<T> Clone for CSlice<'static, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for OrphanPtr<T> {}
impl<T> Clone for OrphanPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> core::fmt::Debug for OrphanPtr<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OrphanPtr")
            .field("addr", &self.addr)
            .finish()
    }
}
impl<T> core::fmt::Debug for CSlice<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CSlice")
            .field("ptr", &self.ptr)
            .field("length", &self.length)
            .finish()
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct CSliceMut<'a, T> {
    ptr: *mut T,
    length: usize,
    _cont: PhantomData<&'a mut [T]>,
}

impl<'a, T> CSlice<'a, T> {
    pub unsafe fn from_raw_parts(ptr: *const T, length: usize) -> Self {
        Self {
            ptr,
            length: Unalign::new(length),
            _cont: PhantomData,
        }
    }

    pub fn from_slice(s: &'a [T]) -> Self {
        unsafe { Self::from_raw_parts(s.as_ptr(), s.len()) }
    }

    pub fn ptr(&self) -> *const T {
        self.ptr
    }

    pub fn len(&self) -> usize {
        self.length.into_inner()
    }

    pub fn into_slice(self) -> &'a [T] {
        unsafe { core::slice::from_raw_parts(self.ptr, self.len()) }
    }
}

impl<'a, T> CSliceMut<'a, T> {
    pub fn from_slice(s: &'a mut [T]) -> Self {
        Self {
            ptr: s.as_mut_ptr(),
            length: s.len(),
            _cont: PhantomData,
        }
    }

    pub fn ptr(&self) -> *mut T {
        self.ptr
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn into_slice(self) -> &'a mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.length) }
    }
}
