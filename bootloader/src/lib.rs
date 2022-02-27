//! UEFI System functionalities.
#![no_std]
#![feature(maybe_uninit_slice)]
#![feature(alloc_error_handler)]
#![feature(ptr_as_uninit)]
#![feature(nonnull_slice_from_raw_parts)]
#![feature(slice_ptr_len)]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_debug_implementations)]
#![warn(missing_copy_implementations)]
#![warn(missing_docs)]

extern crate alloc as alloc_api;

pub mod sys;
