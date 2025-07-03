#[repr(C)]
struct CSlice<'a, T> {
    ptr: *const T,
    length: usize,
    _cont: PhantomData<&'a [T]>,
}
#[repr(C)]
struct CSliceMut<'a, T> {
    ptr: *mut T,
    length: usize,
    _cont: PhantomData<&'a mut [T]>,
}

impl<'a, T> CSlice<'a, T> {
    pub fn into_slice(self) -> &'a [T] {
        unsafe { core::slice::from_raw_parts(self.ptr, self.length) }
    }
}

impl<'a, T> CSliceMut<'a, T> {
    pub fn into_slice(self) -> &'a mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.length) }
    }
}
