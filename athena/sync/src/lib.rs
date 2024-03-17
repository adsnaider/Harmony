#![cfg_attr(not(test), no_std)]

pub mod atomic_ref_cell;
pub use atomic_ref_cell::{AtomicRefCell, BorrowError, Ref, RefMut};
