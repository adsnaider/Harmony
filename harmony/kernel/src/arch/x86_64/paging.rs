pub const PAGE_SIZE: usize = 4096;
pub const FRAME_SIZE: u64 = 4096;

pub mod frames;
pub use frames::RawFrame;

pub mod pages;
pub use pages::Page;

pub mod page_table;

pub mod physical_address;
pub use physical_address::PhysAddr;

pub mod virtual_address;
pub use virtual_address::VirtAddr;
