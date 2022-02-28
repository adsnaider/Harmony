//! Functionalities for displays and frames.
#![cfg_attr(not(test), no_std)]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_debug_implementations)]
#![warn(missing_copy_implementations)]
#![warn(missing_docs)]

#[cfg(target_os = "linux")]
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert!(true);
    }
}
