//! Memory retyping implementation

#[repr(C)]
#[derive(Debug, Clone)]
pub struct MemoryBlock {
    physical_start: u64,
    pages: u32,
    rc: ReferenceCount,
}

#[repr(u8)]
#[derive(Debug, Clone)]
pub enum ReferenceCount {
    Unused,
    User(u16),
    Kernel(u16),
}
