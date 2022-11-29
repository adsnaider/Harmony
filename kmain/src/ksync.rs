//! Kernel asynchronous tools and execution.
//!
//! The `ksync` module provides the asynchronous runtime used by the kernel to provide parallel
//! execution at I/O bounds with interrupts. It additionally provides the low-level futures
//! required to start awaiting on I/O.

pub mod executor;
