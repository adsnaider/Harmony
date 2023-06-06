//! Synchronization utilities for preempted-execution.

pub mod mutex;
pub mod sem;

pub use mutex::Mutex;
pub use sem::Semaphore;
