//! Memory allocation and paging utilities.
#![cfg_attr(not(test), no_std)]
#![feature(allocator_api)]
#![feature(nonnull_slice_from_raw_parts)]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]

pub mod allocation;

#[cfg(test)]
pub(crate) mod test_utils {
    pub fn init_logging() {
        let _ = env_logger::builder().is_test(true).try_init();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        crate::test_utils::init_logging();
        log::info!("Hello world!");
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
