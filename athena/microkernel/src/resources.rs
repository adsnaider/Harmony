//! Kernel datastructures that fit into a page.

use core::ptr::NonNull;

use crate::arch::paging::PAGE_SIZE;

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
union Resource {
    whatever: u32,
}

/// Tag to describe a resource.
#[repr(u8)]
#[derive(Debug, Clone)]
pub enum ResourceType {
    Whatever,
}

#[repr(transparent)]
pub struct KPtr<T> {
    inner: NonNull<T>,
}

impl<T> KPtr<T> {
    const _SIZE_AND_ALIGN: () = {
        assert!(core::mem::size_of::<T>() == PAGE_SIZE);
        assert!(core::mem::align_of::<T>() == PAGE_SIZE);
    };

    pub fn new(value: T) -> Self {
        todo!();
    }
}

impl<T> Clone for KPtr<T> {
    fn clone(&self) -> Self {
        todo!()
    }
}

const _RESOURCE_SIZE_AND_ALIGN: () = {
    assert!(core::mem::size_of::<Resource>() == PAGE_SIZE);
    assert!(core::mem::align_of::<Resource>() == PAGE_SIZE);
};
