//! Process loading and execution.
use core::arch::asm;

use goblin::elf64::header::{Header, SIZEOF_EHDR};
use goblin::elf64::program_header::{ProgramHeader, PF_R, PF_W, PF_X, PT_LOAD};
use thiserror::Error;

use crate::arch::mm::addrspace::{AddrSpace, PageTableFlags, VirtPage};
use crate::arch::mm::frames::FrameBumpAllocator;
use crate::arch::{sysret, PRIVILEGE_STACK_ADDR};

/// Initializes the syscall/sysret operation for switching to ring 0/3.
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

struct Segment<'prog, 'head> {
    program: &'prog [u8],
    header: &'head ProgramHeader,
}

impl<'prog, 'head> Segment<'prog, 'head> {
    pub fn new(program: &'prog [u8], header: &'head ProgramHeader) -> Self {
        Self { program, header }
    }

    pub fn load(&self, address_space: &mut AddrSpace, fallocator: &mut FrameBumpAllocator) {
        let vm_range = self.header.p_vaddr..(self.header.p_vaddr + self.header.p_memsz);
        let file_range = self.header.p_offset..(self.header.p_offset + self.header.p_filesz);

        assert!(vm_range.end <= 0xFFFF800000000000);
        assert!(file_range.end <= self.program.len() as u64);
        assert!(self.header.p_memsz >= self.header.p_filesz);
        let mut vcurrent = vm_range.start;
        let mut fcurrent = file_range.start;
        while vcurrent < vm_range.end {
            let frame = fallocator.alloc_frame().unwrap().into_user().into_raw();
            let page = VirtPage::containing_address(vcurrent);
            // SAFETY: Just mapping the elf data.
            let flags = self.header.p_flags;
            let mut pflags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
            assert!(flags & PF_R != 0);
            if flags & PF_W != 0 {
                pflags |= PageTableFlags::WRITABLE;
            }
            if flags & PF_X == 0 {
                pflags |= PageTableFlags::NO_EXECUTE;
            }
            unsafe {
                address_space
                    .map_to(page, frame, pflags, fallocator)
                    .unwrap()
            }

            let offset_page: *mut u8 = frame.as_ptr_mut();

            let count = usize::min(
                (file_range.end - fcurrent) as usize,
                4096 - vcurrent as usize % 4096,
            );

            // SAFETY: Hopefully no bugs here...
            unsafe {
                core::ptr::write_bytes(offset_page, 0, 4096);
                if count > 0 {
                    core::ptr::copy(
                        self.program.as_ptr().add(fcurrent as usize),
                        offset_page.add(vcurrent as usize % 4096),
                        count,
                    )
                }
            }
            vcurrent += 4096 - vcurrent % 4096;
            fcurrent += count as u64;
        }
    }
}

impl Process {
    /// Loads the process to main memory and returns a context associated with it.
    pub fn load(
        program: &[u8],
        stack_pages: u64,
        fallocator: &mut FrameBumpAllocator,
    ) -> Result<Self, LoadError> {
        // FIXME: This should create a new thread/component alltogether!
        let mut addrspace = AddrSpace::new(fallocator.alloc_frame().unwrap());
        let header = Header::from_bytes(program[..SIZEOF_EHDR].try_into().unwrap());
        let entry = header.e_entry;
        log::trace!("Entry: {:X}", entry);
        let phdrs = unsafe {
            assert!(
                program.len()
                    > usize::try_from(header.e_phoff).unwrap()
                        + usize::try_from(header.e_phentsize).unwrap()
                            * usize::try_from(header.e_phnum).unwrap()
            );
            let phdr_start: *const ProgramHeader = program
                .as_ptr()
                .add(header.e_phoff.try_into().unwrap())
                .cast();
            assert!(phdr_start as usize % core::mem::align_of::<ProgramHeader>() == 0);
            ProgramHeader::from_raw_parts(phdr_start, header.e_phnum.try_into().unwrap())
        };
        for ph in phdrs {
            if ph.p_type == PT_LOAD {
                let segment = Segment::new(program, ph);
                segment.load(&mut addrspace, fallocator);
            }
        }
        let rsp = 0x0000_8000_0000_0000;

        for i in 0..stack_pages {
            let frame = fallocator.alloc_frame().unwrap().into_user().into_raw();
            let addr = rsp - 4096 * (i + 1);
            let page = VirtPage::from_start_address(addr).unwrap();
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

        let interrupt_stack = fallocator.alloc_frame().unwrap().into_kernel().into_raw();
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
