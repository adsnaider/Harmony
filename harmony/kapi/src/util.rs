use core::marker::PhantomData;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CSlice<'a, T> {
    ptr: *const T,
    length: usize,
    _cont: PhantomData<&'a [T]>,
}
#[repr(C)]
#[derive(Debug)]
pub struct CSliceMut<'a, T> {
    ptr: *mut T,
    length: usize,
    _cont: PhantomData<&'a mut [T]>,
}

impl<'a, T> CSlice<'a, T> {
    pub fn from_slice(s: &'a [T]) -> Self {
        Self {
            ptr: s.as_ptr(),
            length: s.len(),
            _cont: PhantomData,
        }
    }

    pub fn into_slice(self) -> &'a [T] {
        unsafe { core::slice::from_raw_parts(self.ptr, self.length) }
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

    pub fn into_slice(self) -> &'a mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.length) }
    }
}
