//! Lock-free, wait-free synchronization primitives

mod atomic_ref_cell;
pub use atomic_ref_cell::{AtomicRefCell, BorrowError, Ref, RefMut};
