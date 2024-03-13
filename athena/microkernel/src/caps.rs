/// Actual under-the-hood implementation for capability tables
mod trie;

#[repr(C)]
pub struct Capability {
    resource: KPtr<Resource>,
    resource_type: ResourceType,
    flags: CapFlags,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CapFlags(u16);

#[repr(transparent)]
pub struct KPtr<T> {
    inner: *mut T,
}
