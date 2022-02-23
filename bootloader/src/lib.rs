//! UEFI System functionalities.
#![no_std]
#![feature(alloc_error_handler)]
#![warn(unused_crate_dependencies)]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_debug_implementations)]
#![warn(missing_copy_implementations)]
#![warn(missing_docs)]

extern crate alloc as alloc_api;

pub mod sys;
