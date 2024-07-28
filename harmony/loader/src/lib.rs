#![no_std]

use core::mem::MaybeUninit;
use core::ops::Range;

use bitflags::bitflags;
use goblin::elf::program_header::{PF_R, PF_W, PF_X};
pub use goblin::elf64;
use goblin::elf64::header::{Header, SIZEOF_EHDR};
use goblin::elf64::program_header::{ProgramHeader, PT_LOAD};

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[derive(Debug)]
pub struct Program<'a> {
    program: &'a [u8],
    header: &'a Header,
    program_headers: &'a [ProgramHeader],
}

#[derive(Debug)]
pub struct Process {
    pub entry: u64,
}

#[derive(Debug, Copy, Clone)]
pub enum Error {
    BadElf,
    InvalidOffsets,
    NotAligned,
}

impl<'a> Program<'a> {
    pub fn new(program: &'a [u8]) -> Result<Self, Error> {
        if program.as_ptr() as usize % 16 != 0 {
            return Err(Error::NotAligned);
        }
        let header = Header::from_bytes(
            program[..SIZEOF_EHDR]
                .try_into()
                .map_err(|_| Error::BadElf)?,
        );
        let program_headers = {
            let phoff: usize = header.e_phoff.try_into().map_err(|_| Error::BadElf)?;
            let entry_size: usize = header.e_phentsize.into();
            let entries: usize = header.e_phnum.into();

            let length = entry_size
                .checked_mul(entries)
                .ok_or(Error::InvalidOffsets)?;
            let end = phoff.checked_add(length).ok_or(Error::InvalidOffsets)?;
            if end > program.len() {
                return Err(Error::InvalidOffsets);
            }

            unsafe {
                let phdr_start: *const ProgramHeader = program.as_ptr().add(phoff).cast();
                ProgramHeader::from_raw_parts(phdr_start, entries)
            }
        };
        Ok(Self {
            program,
            header,
            program_headers,
        })
    }

    pub fn load<L>(&self, loader: &mut L) -> Result<Process, SegmentLoadError>
    where
        L: SegmentLoader,
    {
        match self.load_headers(loader) {
            Ok(_) => Ok(Process {
                entry: self.header.e_entry,
            }),
            Err((loaded_count, e)) => {
                // SAFETY: This segment was previously loaded and will only be unloaded once.
                unsafe { self.unload_headers(loader, loaded_count) };
                Err(e)
            }
        }
    }

    fn load_headers<L: SegmentLoader>(
        &self,
        loader: &mut L,
    ) -> Result<(), (usize, SegmentLoadError)> {
        for (i, phdr) in self
            .program_headers
            .iter()
            .filter(|phdr| phdr.p_type == PT_LOAD)
            .enumerate()
        {
            let segment = Segment {
                program: self.program,
                header: phdr,
            };
            loader.load(segment).map_err(|e| (i, e))?;
        }
        Ok(())
    }

    /// # Safety
    ///
    /// The count must accurately track currently loaded segments.
    unsafe fn unload_headers<L: SegmentLoader>(&self, loader: &mut L, count: usize) {
        for phdr in self
            .program_headers
            .iter()
            .filter(|phdr| phdr.p_type == PT_LOAD)
            .take(count)
        {
            let segment = Segment {
                program: self.program,
                header: phdr,
            };
            // SAFETY: Preconditions.
            unsafe { loader.unload(segment) };
        }
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct MemFlags: u8 {
        const READ = 0x0001;
        const WRITE = 0x0002;
        const EXECUTE = 0x0004;
    }
}

impl MemFlags {
    pub fn readable(&self) -> bool {
        self.contains(MemFlags::READ)
    }

    pub fn writeable(&self) -> bool {
        self.contains(MemFlags::WRITE)
    }

    pub fn executable(&self) -> bool {
        self.contains(MemFlags::EXECUTE)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SegmentLoadError {
    InvalidFileRange,
    InvalidVirtualRange,
    FileRangeLargerThanVirtualRange,
    RangeOverflow,
    BadMemoryFlags,
}

fn range_convert<T, U, E>(range: Range<T>) -> Result<Range<U>, E>
where
    U: TryFrom<T, Error = E>,
{
    let start = range.start.try_into()?;
    let end = range.end.try_into()?;
    Ok(Range { start, end })
}

pub trait SegmentLoader {
    /// Requests a virtual memory range mapping from the loader
    ///
    /// The behavior of this function is one in which the returned slice in the Ok case
    /// should directly mapped to the virtual memory range of the process/segment being
    /// loaded. The resulting slice must be the same length as the requested range or
    /// the load implementation may panic.
    fn request_virtual_memory_range(
        &mut self,
        range: Range<usize>,
        rwx: MemFlags,
    ) -> Result<&mut [MaybeUninit<u8>], SegmentLoadError>;

    /// Releases the virtual memory range previously requested
    ///
    /// # Safety
    ///
    /// The caller must have requested the range with `request_virtual_memory_range` which must
    /// have returned `Ok`. This function may only be called once per requested memory range.
    unsafe fn release_virtual_memory_range(&mut self, range: Range<usize>);

    fn load(&mut self, segment: Segment<'_, '_>) -> Result<(), SegmentLoadError> {
        let vm_range = range_convert(
            segment.header.p_vaddr
                ..(segment
                    .header
                    .p_vaddr
                    .checked_add(segment.header.p_memsz)
                    .ok_or(SegmentLoadError::RangeOverflow)?),
        )
        .map_err(|_| SegmentLoadError::InvalidVirtualRange)?;
        let file_range: Range<usize> = range_convert(
            segment.header.p_offset
                ..(segment
                    .header
                    .p_offset
                    .checked_add(segment.header.p_filesz)
                    .ok_or(SegmentLoadError::RangeOverflow)?),
        )
        .map_err(|_| SegmentLoadError::InvalidFileRange)?;

        let flags = segment.header.p_flags;
        let mut mem_flags = MemFlags::empty();
        if flags & PF_R != 0 {
            mem_flags |= MemFlags::READ;
        }
        if flags & PF_W != 0 {
            mem_flags |= MemFlags::WRITE;
        }
        if flags & PF_X != 0 {
            mem_flags |= MemFlags::EXECUTE;
        }

        let dest = self.request_virtual_memory_range(vm_range.clone(), mem_flags)?;
        assert_eq!(
            dest.len(),
            vm_range.len(),
            "Requested virtual memory range doesn't match expected size"
        );
        // SAFETY: It's okay to zero a byte slice.
        let dest = unsafe {
            core::ptr::write_bytes(dest.as_mut_ptr(), 0, dest.len());
            &mut *(dest as *mut [MaybeUninit<u8>] as *mut [u8])
        };

        let source = segment
            .program
            .get(file_range)
            .ok_or(SegmentLoadError::InvalidFileRange)?;

        if source.len() > dest.len() {
            return Err(SegmentLoadError::FileRangeLargerThanVirtualRange);
        }
        for (source, dest) in source.iter().zip(dest.iter_mut()) {
            unsafe { core::ptr::write_volatile(dest as *mut u8, *source) };
        }
        Ok(())
    }

    /// Unloads the loaded segment, releasing the resources utilized
    ///
    /// # Safety
    ///
    /// The segment must have been loaded with this loader and it may not
    /// have been unloaded already. This has similar semantics to alloc/free.
    unsafe fn unload(&mut self, segment: Segment<'_, '_>) {
        // NOTE: We don't need to do checked arithmetic here because the segment was previously
        // loaded so the ranges are proper.
        let vm_range = segment.header.p_vaddr as usize
            ..(segment.header.p_vaddr as usize + segment.header.p_memsz as usize);

        // SAFETY: Preconditions guarantee this will only be called once per loaded segment.
        unsafe {
            self.release_virtual_memory_range(vm_range);
        }
    }
}

pub struct Segment<'prog, 'head> {
    pub program: &'prog [u8],
    pub header: &'head ProgramHeader,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
