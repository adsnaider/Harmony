//! UEFI System functionalities.
#![no_std]
#![feature(maybe_uninit_write_slice)]
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

pub(crate) mod mem;
pub mod sys;

use uefi::table::boot::MemoryType;

/// UEFI memory type used to represent kernel's statics memory region.
pub const KERNEL_STATIC_MEMORY: MemoryType = MemoryType::custom(bootinfo::KERNEL_STATIC);
/// UEFI memory type used to represent the kernel's stack memory region.
pub const KERNEL_STACK_MEMORY: MemoryType = MemoryType::custom(bootinfo::KERNEL_STACK);
/// UEFI memory type used to represent the kernel's code memory region.
pub const KERNEL_CODE_MEMORY: MemoryType = MemoryType::custom(bootinfo::KERNEL_CODE);
