//! Simple filesystem API to read and write files within the UEFI environment.

use alloc_api::string::String;
use alloc_api::vec::Vec;
use alloc_api::{format, vec};
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileType};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::CStr16;

use crate::sys::{GlobalTable, SYSTEM_TABLE};

#[derive(Copy, Clone, Debug)]
/// Filesystem errors.
pub struct Error {}

/// Reads the content of the file in `path` into a `Vec<u8>`
pub fn read(path: &str) -> Result<Vec<u8>, Error> {
    let table = SYSTEM_TABLE.get();
    let mut fs = GlobalTable::open_protocol::<SimpleFileSystem>(&table)
        .expect("Unable to open SimpleFileSystem protocol");

    let mut walker = fs.open_volume().expect("Can't open volume.");

    let mut it = path.split('/').peekable();
    while let Some(entry) = it.next() {
        let mut cstr_buffer = [0; 32];
        let next = walker
            .open(
                CStr16::from_str_with_buf(entry, &mut cstr_buffer)
                    .expect("Invalid filename should be CStr16"),
                uefi::proto::media::file::FileMode::Read,
                FileAttribute::empty(),
            )
            .expect(&format!("Can't open file {}", path))
            .into_type()
            .expect("Can't get file type.");

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
                    let info: &FileInfo = file.get_info(&mut buf).expect("Couldn't get file info");
                    info.file_size() as usize
                };
                let mut data = vec![0; size];
                file.read(&mut data)
                    .expect(&format!("Couldn't read file: {}", path));
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
