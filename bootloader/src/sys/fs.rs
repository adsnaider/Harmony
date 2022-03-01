//! Simple filesystem API to read and write files within the UEFI environment.

use alloc_api::format;
use alloc_api::string::String;
use alloc_api::vec::Vec;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileType};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::ResultExt;

use crate::sys::SYSTEM_TABLE;

#[derive(Copy, Clone, Debug)]
/// Filesystem errors.
pub struct Error {}

/// Reads the content of the file in `path` into a `Vec<u8>`
pub fn read(path: &str) -> Result<Vec<u8>, Error> {
    let fs: &mut SimpleFileSystem = unsafe {
        &mut *SYSTEM_TABLE
            .get_mut()
            .boot_services()
            .locate_protocol()
            .expect_success("Can't open filesystem.")
            .get()
    };

    let mut walker = fs.open_volume().expect_success("Can't open volume.");
    let mut it = path.split('/').peekable();
    while let Some(entry) = it.next() {
        let next = walker
            .open(
                entry,
                uefi::proto::media::file::FileMode::Read,
                FileAttribute::empty(),
            )
            .expect_success(&format!("Can't open file {}", path))
            .into_type()
            .expect_success("Can't get file type.");

        match next {
            FileType::Regular(mut file) => {
                if it.peek().is_some() {
                    // Found the file before the end of the path.
                    return Err(Error {});
                }
                let size: usize = {
                    // TODO(#3): Cheating by creating a fixed-size buffer. This is a common
                    // problem. Maybe make something to read into a Box<[u8]> for all of these
                    // cases.
                    let mut buf: [u8; 128] = [0; 128];
                    let info: &FileInfo = file
                        .get_info(&mut buf)
                        .expect_success("Couldn't get file info");
                    info.file_size() as usize
                };
                let mut data = Vec::with_capacity(size);
                data.resize(size, 0);
                file.read(&mut data)
                    .expect_success(&format!("Couldn't read file: {}", path));
                return Ok(data);
            }
            FileType::Dir(directory) => walker = directory,
        }
    }
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
