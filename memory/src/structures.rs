//! Physical memory structures.
pub mod frame;
pub mod page;

pub use self::frame::Frame;
pub use self::page::Page;

/// Represents the size of a page.
pub trait PageSize {
    /// The size in bytes of the page/frame.
    const SIZE: usize;
}

/// 4KiB page.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Size4KiB {}
impl PageSize for Size4KiB {
    const SIZE: usize = 4 * 1024;
}

/// 2MiB large page.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Size2MiB {}
impl PageSize for Size2MiB {
    const SIZE: usize = 2 * 1024 * 1024;
}

/// 1GiB huge page.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Size1GiB {}
impl PageSize for Size1GiB {
    const SIZE: usize = 1024 * 1024 * 1024;
}
