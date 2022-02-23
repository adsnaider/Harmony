//! Simple filesystem API to read and write files within the UEFI environment.
use alloc_api::string::String;
use alloc_api::vec::Vec;

#[derive(Copy, Clone, Debug)]
/// Filesystem errors.
pub struct Error {}

/// Reads the content of the file in `path` into a `Vec<u8>`
pub fn read(_path: &str) -> Result<Vec<u8>, Error> {
    todo!();
}

/// Writes the `contents` into the file in `path`
pub fn write(_path: &str, _contents: &[u8]) -> Result<(), Error> {
    todo!();
}

/// Returns a list of the files and directories that can be found within `path`.
pub fn read_dir(_path: &str) -> Result<Vec<String>, Error> {
    todo!();
}
