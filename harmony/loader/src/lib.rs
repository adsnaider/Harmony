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
    entry: u64,
    top_of_text: usize,
}

impl Process {
    pub fn entry(&self) -> u64 {
        self.entry
    }

    /// Returns the virtual address denoting the top of the .text section
    pub fn top_of_text(&self) -> usize {
        self.top_of_text
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ElfError {
    BadElf,
    InvalidOffsets,
    NotAligned,
}

impl<'a> Program<'a> {
    /// Parses a new program, potentially returning an error if the ELF couldn't be parsed.
    pub fn new(program: &'a [u8]) -> Result<Self, ElfError> {
        if program.as_ptr() as usize % 16 != 0 {
            return Err(ElfError::NotAligned);
        }
        let header = Header::from_bytes(
            program[..SIZEOF_EHDR]
                .try_into()
                .map_err(|_| ElfError::BadElf)?,
        );
        let program_headers = {
            let phoff: usize = header.e_phoff.try_into().map_err(|_| ElfError::BadElf)?;
            let entry_size: usize = header.e_phentsize.into();
            let entries: usize = header.e_phnum.into();

            let length = entry_size
                .checked_mul(entries)
                .ok_or(ElfError::InvalidOffsets)?;
            let end = phoff.checked_add(length).ok_or(ElfError::InvalidOffsets)?;
            if end > program.len() {
                return Err(ElfError::InvalidOffsets);
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

    // TODO: Implement non-static PIE binaries and shared libraries.
    /// Loads the program using the provided loader implementation.
    pub fn load<L>(&self, loader: &mut L) -> Result<Process, SegmentLoadError<L::Error>>
    where
        L: Loader,
    {
        match self.load_headers(loader) {
            Ok(top_of_text) => Ok(Process {
                entry: self.header.e_entry,
                top_of_text,
            }),
            Err((loaded_count, e)) => {
                // SAFETY: This segment was previously loaded and will only be unloaded once.
                unsafe { self.unload_headers(loader, loaded_count) };
                Err(e)
            }
        }
    }

    fn load_headers<L: Loader>(
        &self,
        loader: &mut L,
    ) -> Result<usize, (usize, SegmentLoadError<L::Error>)> {
        let mut top_of_text = 0;
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
            let range = segment.load(loader).map_err(|e| (i, e))?;
            top_of_text = usize::max(top_of_text, range.end);
        }
        Ok(top_of_text)
    }

    /// # Safety
    ///
    /// The count must accurately track currently loaded segments.
    unsafe fn unload_headers<L: Loader>(&self, loader: &mut L, count: usize) {
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
            unsafe { segment.unload(loader) };
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
pub enum SegmentLoadError<E> {
    LoaderError(E),
    InvalidFileRange,
    InvalidVirtualRange,
    SourceLargerThanVirtualRange,
    RangeOverflow,
}

impl<E> From<E> for SegmentLoadError<E> {
    fn from(value: E) -> Self {
        SegmentLoadError::LoaderError(value)
    }
}

fn range_convert<T, U, E>(range: Range<T>) -> Result<Range<U>, E>
where
    U: TryFrom<T, Error = E>,
{
    let start = range.start.try_into()?;
    let end = range.end.try_into()?;
    Ok(Range { start, end })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderError<E> {
    LoaderError(E),
    SourceLargerThanVirtualRange,
}

impl<E> From<E> for LoaderError<E> {
    fn from(value: E) -> Self {
        Self::LoaderError(value)
    }
}

impl<E> From<LoaderError<E>> for SegmentLoadError<E> {
    fn from(value: LoaderError<E>) -> Self {
        match value {
            LoaderError::LoaderError(e) => Self::LoaderError(e),
            LoaderError::SourceLargerThanVirtualRange => Self::SourceLargerThanVirtualRange,
        }
    }
}

/// A general-purpose Loader trait to enable loading programs
pub trait Loader {
    type Error;

    /// Loads bytes by using a "mapping" source function that provides the bytes to load.
    ///
    /// The `source` is a more general, array-like structure that returns the byte to load
    /// at a specific address. The implementation will load the range
    /// `source(0..count) -> at.start..at.end`. The `rwx` provides the protection bitmap that
    /// the loader should use for this entire chunk.
    fn load_with<F>(
        &mut self,
        at: Range<usize>,
        source: F,
        rwx: MemFlags,
    ) -> Result<(), Self::Error>
    where
        F: Fn(usize) -> MaybeUninit<u8>;

    /// Loads the `source` input into the VM range denoted by `at`.
    ///
    /// If the virtual range is larger than the source, the source will be loaded
    /// to the beginning of `at` range and the remaining bits will be zeroed
    fn load_source(
        &mut self,
        at: Range<usize>,
        source: &[u8],
        rwx: MemFlags,
    ) -> Result<(), LoaderError<Self::Error>> {
        if source.len() > at.len() {
            return Err(LoaderError::SourceLargerThanVirtualRange);
        }
        self.load_with(
            at,
            |i| MaybeUninit::new(source.get(i).copied().unwrap_or(0)),
            rwx,
        )?;
        Ok(())
    }

    /// Loads zeros to the entire range.
    fn load_zeroed(&mut self, at: Range<usize>, rwx: MemFlags) -> Result<(), Self::Error> {
        self.load_with(at, |_| MaybeUninit::new(0), rwx)?;
        Ok(())
    }

    /// Loads an entire range without assigning any specific values to the loaded range.
    fn load_uninit(&mut self, at: Range<usize>, rwx: MemFlags) -> Result<(), Self::Error> {
        self.load_with(at, |_| MaybeUninit::uninit(), rwx)?;
        Ok(())
    }

    /// Releases the virtual memory range previously loaded.
    ///
    /// # Safety
    ///
    /// The caller must have requested the range with `load_source` which must
    /// have returned `Ok`. This function may only be called once per requested memory range.
    unsafe fn unload(&mut self, vrange: Range<usize>);
}

struct Segment<'prog, 'head> {
    pub program: &'prog [u8],
    pub header: &'head ProgramHeader,
}

impl<'prog, 'head> Segment<'prog, 'head> {
    /// Loads the segment using the loader implementation.
    pub fn load<L: Loader>(
        &self,
        loader: &mut L,
    ) -> Result<Range<usize>, SegmentLoadError<L::Error>> {
        let vm_range = range_convert(
            self.header.p_vaddr
                ..(self
                    .header
                    .p_vaddr
                    .checked_add(self.header.p_memsz)
                    .ok_or(SegmentLoadError::RangeOverflow)?),
        )
        .map_err(|_| SegmentLoadError::InvalidVirtualRange)?;
        let file_range: Range<usize> = range_convert(
            self.header.p_offset
                ..(self
                    .header
                    .p_offset
                    .checked_add(self.header.p_filesz)
                    .ok_or(SegmentLoadError::RangeOverflow)?),
        )
        .map_err(|_| SegmentLoadError::InvalidFileRange)?;

        let flags = self.header.p_flags;
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
        log::debug!("Loading segment at {:X?} with {:?}", &vm_range, &mem_flags);
        let source = self
            .program
            .get(file_range)
            .ok_or(SegmentLoadError::InvalidFileRange)?;
        loader.load_source(vm_range.clone(), source, mem_flags)?;
        Ok(vm_range)
    }

    /// Unloads the loaded segment, releasing the resources utilized
    ///
    /// # Safety
    ///
    /// The segment must have been loaded with this loader and it may not
    /// have been unloaded already. This has similar semantics to alloc/free.
    unsafe fn unload<L: Loader>(&self, loader: &mut L) {
        // NOTE: We don't need to do checked arithmetic here because the segment was previously
        // loaded so the ranges are proper.
        let vm_range = self.header.p_vaddr as usize
            ..(self.header.p_vaddr as usize + self.header.p_memsz as usize);

        // SAFETY: Preconditions guarantee this will only be called once per loaded segment.
        unsafe {
            loader.unload(vm_range);
        }
    }
}
