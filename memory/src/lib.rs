//! Memory allocation and paging utilities.
#![cfg_attr(not(test), no_std)]
#![feature(allocator_api)]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]

pub mod allocation;

#[cfg(target_os = "linux")]
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
