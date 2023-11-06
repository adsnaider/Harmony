//! Process loading and execution.
use core::arch::asm;

use goblin::elf::program_header::PT_LOAD;
use goblin::elf::{Elf, ProgramHeader};
use thiserror::Error;

use crate::arch::mm::paging::{AddrSpace, PageTableFlags};
use crate::arch::mm::{Frame, VirtPage};
use crate::arch::sysret;

/// Initializes the sysret operation for switching to ring 3.
pub fn init() {
    sce_enable();
}

/// A process is a thread with its own memory space that runs in Ring 3.
#[derive(Debug)]
pub struct Process {
    entry: u64,
}

/// Error loading the ELF binary.
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("Error parsing the binary")]
    /// Error during parsing.
    ParseError(goblin::error::Error),
}

impl From<goblin::error::Error> for LoadError {
    fn from(value: goblin::error::Error) -> Self {
        Self::ParseError(value)
    }
}

struct Segment<'a> {
    program: &'a [u8],
    file_off: u64,
    virt_addr: u64,
    filesz: u64,
    memsz: u64,
}

impl<'a> Segment<'a> {
    pub fn new(program: &'a [u8], header: &ProgramHeader) -> Self {
        Self {
            program,
            file_off: header.p_offset,
            virt_addr: header.p_vaddr,
            filesz: header.p_filesz,
            memsz: header.p_memsz,
        }
    }

    pub fn load(&self, address_space: &mut AddrSpace) {
        let vm_range = self.virt_addr..(self.virt_addr + self.memsz);
        let file_range = self.file_off..(self.file_off + self.filesz);

        // FIXME: No panics!
        assert!(vm_range.end <= 0xFFFF800000000000);
        assert!(vm_range.start % 4096 == 0);
        assert!(file_range.end <= self.program.len() as u64);
        assert!(self.memsz >= self.filesz);
        let frames_needed = (self.memsz - 1) / 4096 + 1;
        let mut remaining = self.filesz as usize;
        for i in 0..frames_needed {
            let frame = Frame::alloc().unwrap();
            let page = VirtPage::from_start_address(vm_range.start + i * 4096).unwrap();
            // SAFETY: Just mapping the elf data.
            unsafe {
                address_space
                    .map_to(
                        page,
                        frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::USER_ACCESSIBLE
                            | PageTableFlags::WRITABLE,
                    )
                    .unwrap()
            }

            let offset_page = frame.physical_offset().as_mut_ptr();
            // SAFETY: Hopefully no bugs here...
            unsafe {
                core::ptr::write_bytes(offset_page, 0, 4096);
                core::ptr::copy(
                    self.program
                        .as_ptr()
                        .add((self.file_off + i * 4096) as usize),
                    offset_page,
                    usize::min(remaining, 4096),
                )
            }
            remaining -= usize::min(remaining, 4096);
        }
    }
}

impl Process {
    /// Loads the process to main memory and returns a context associated with it.
    pub fn load(
        program: &[u8],
        stack_pages: u64,
        addrspace: &mut AddrSpace,
    ) -> Result<Self, goblin::error::Error> {
        let elf = Elf::parse(program)?;
        assert!(elf.is_64);
        for ph in elf.program_headers.iter().filter(|ph| ph.p_type == PT_LOAD) {
            let segment = Segment::new(program, ph);
            segment.load(addrspace);
        }
        let rsp = 0x0000_8000_0000_0000;

        for i in 0..stack_pages {
            let frame = Frame::alloc().unwrap();
            let addr = rsp - 4096 * (i + 1);
            let page = VirtPage::from_start_address(addr).unwrap();
            log::debug!("Mapping page {page:?} to frame {frame:?}");
            // SAFETY: Just mapping the stack pages.
            unsafe {
                addrspace
                    .map_to(
                        page,
                        frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::USER_ACCESSIBLE
                            | PageTableFlags::NO_EXECUTE,
                    )
                    .unwrap()
            }
        }
        Ok(Self { entry: elf.entry })
    }

    /// Runs the user process.
    ///
    /// # Safety
    ///
    /// The correct address space must be active.
    pub unsafe fn exec(&self) -> ! {
        log::debug!("Entering process at {:#X}", self.entry);
        // SAFETY: The entry and rsp are valid for the user process.
        unsafe {
            sysret(self.entry, 0x0000_8000_0000_0000);
        }
    }
}

fn sce_enable() {
    // SAFETY: Nothing special, just enabling Syscall extension.
    unsafe {
        asm!(
            "mov rcx, 0xc0000082",
            "wrmsr",
            "mov rcx, 0xc0000080",
            "rdmsr",
            "or eax, 1",
            "wrmsr",
            "mov rcx, 0xc0000081",
            "rdmsr",
            "mov edx, 0x00180008",
            "wrmsr",
            out("rcx") _,
            out("eax") _,
            out("edx") _,
            options(nostack, nomem),
        );
    }
}
