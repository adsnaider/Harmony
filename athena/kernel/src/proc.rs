//! Process loading and execution.
use core::arch::asm;

use goblin::elf64::header::{Header, SIZEOF_EHDR};
use goblin::elf64::program_header::ProgramHeader;
use thiserror::Error;

use crate::arch::mm::addrspace::{AddrSpace, PageTableFlags, VirtPage};
use crate::arch::mm::frames::FrameBumpAllocator;
use crate::arch::{sysret, PRIVILEGE_STACK_ADDR};

/// Initializes the sysret operation for switching to ring 3.
pub fn init() {
    sce_enable();
}

/// A process is a thread with its own memory space that runs in Ring 3.
#[derive(Debug)]
pub struct Process {
    entry: u64,
    addrspace: AddrSpace,
}

/// Error loading the ELF binary.
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("Error parsing the binary")]
    /// Error during parsing.
    ParseError,
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

    pub fn load(&self, address_space: &mut AddrSpace, fallocator: &mut FrameBumpAllocator<'_>) {
        let vm_range = self.virt_addr..(self.virt_addr + self.memsz);
        let file_range = self.file_off..(self.file_off + self.filesz);

        assert!(vm_range.end <= 0xFFFF800000000000);
        assert!(vm_range.start % 4096 == 0);
        assert!(file_range.end <= self.program.len() as u64);
        assert!(self.memsz >= self.filesz);
        let frames_needed = (self.memsz - 1) / 4096 + 1;
        let mut remaining = self.filesz as usize;
        for i in 0..frames_needed {
            let frame = fallocator.alloc_frame().unwrap();
            let page = VirtPage::from_start_address(vm_range.start + i * 4096).unwrap();
            // SAFETY: Just mapping the elf data.
            log::debug!("Mapping page {page:?} to frame {frame:?}");
            unsafe {
                address_space
                    .map_to(
                        page,
                        frame,
                        PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
                        fallocator,
                    )
                    .unwrap()
            }

            let offset_page = frame.as_ptr_mut();
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
        fallocator: &mut FrameBumpAllocator<'_>,
    ) -> Result<Self, LoadError> {
        let mut addrspace = AddrSpace::new(fallocator.alloc_frame().unwrap());
        let header = Header::from_bytes(program[..SIZEOF_EHDR].try_into().unwrap());
        let entry = header.e_entry;
        let phdrs = unsafe {
            assert!(program.len() > header.e_phoff.try_into().unwrap());
            assert!(
                program.len()
                    > usize::try_from(header.e_phoff).unwrap()
                        + usize::try_from(header.e_phentsize).unwrap()
                            * usize::try_from(header.e_phnum).unwrap()
            );
            ProgramHeader::from_raw_parts(
                program
                    .as_ptr()
                    .add(header.e_phoff.try_into().unwrap())
                    .cast(),
                header.e_phnum.try_into().unwrap(),
            )
        };
        for ph in phdrs {
            let segment = Segment::new(program, ph);
            segment.load(&mut addrspace, fallocator);
        }
        let rsp = 0x0000_8000_0000_0000;

        for i in 0..stack_pages {
            let frame = fallocator.alloc_frame().unwrap();
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
                        fallocator,
                    )
                    .unwrap()
            }
        }

        let interrupt_stack = fallocator.alloc_frame().unwrap();
        let interrupt_stack_page = VirtPage::from_start_address(PRIVILEGE_STACK_ADDR).unwrap();
        // SAFETY: Interrupt stack page in use will be unnafected since we haven't switched address spaces.
        unsafe {
            let _ = addrspace.unmap(interrupt_stack_page);
            addrspace
                .map_to(
                    interrupt_stack_page,
                    interrupt_stack,
                    PageTableFlags::WRITABLE | PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                    fallocator,
                )
                .unwrap();
        }
        log::debug!("Mapped interrupt stack");
        Ok(Self { entry, addrspace })
    }

    /// Runs the user process.
    pub fn exec(&mut self) -> ! {
        log::debug!("Entering process at {:#X}", self.entry);
        // SAFETY: The entry and rsp are valid for the user process.
        unsafe {
            self.addrspace.activate();
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
