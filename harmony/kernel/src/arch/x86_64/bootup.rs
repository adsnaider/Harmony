//! Boot process initialization

use goblin::elf::program_header::{PF_R, PF_W, PF_X, PT_LOAD};
use goblin::elf64::header::{Header, SIZEOF_EHDR};
use goblin::elf64::program_header::ProgramHeader;

use super::paging::page_table::AnyPageTable;
use crate::arch::exec::{ControlRegs, ExecCtx, Regs};
use crate::arch::paging::page_table::{Addrspace, PageTableFlags};
use crate::arch::paging::{Page, PhysAddr, RawFrame, VirtAddr, FRAME_SIZE, PAGE_SIZE};
use crate::bump_allocator::BumpAllocator;
use crate::kptr::KPtr;

pub struct Process {
    pub entry: u64,
    pub rsp: u64,
    pub l4_table: KPtr<AnyPageTable>,
}

#[derive(Debug)]
pub enum LoadError {}

impl Process {
    pub fn load(
        program: &[u8],
        stack_pages: usize,
        untyped_memory_offset: usize,
        untyped_memory_length: usize,
    ) -> Result<Self, LoadError> {
        let mut fallocator = BumpAllocator::new();
        assert!(untyped_memory_offset % PAGE_SIZE == 0);
        assert!(untyped_memory_length % PAGE_SIZE == 0);
        assert!(untyped_memory_offset + untyped_memory_length < 0xFFFF_8000_0000_0000);
        assert!(
            program.as_ptr() as usize % 16 == 0,
            "ELF must be aligned to 16 bytes"
        );

        log::debug!("Setting up process address space");
        let l4_table = {
            let l4_frame = fallocator.alloc_untyped_frame().unwrap();
            AnyPageTable::new_l4(l4_frame).unwrap()
        };
        let addrspace = unsafe { l4_table.as_addrspace() };
        let header = Header::from_bytes(program[..SIZEOF_EHDR].try_into().unwrap());
        let entry = header.e_entry;
        log::trace!("Entry: {:X}", entry);
        let phdrs = unsafe {
            assert!(
                program.len()
                    > usize::try_from(header.e_phoff).unwrap()
                        + usize::from(header.e_phentsize) * usize::from(header.e_phnum)
            );
            let phdr_start: *const ProgramHeader = program
                .as_ptr()
                .add(header.e_phoff.try_into().unwrap())
                .cast();
            assert!(phdr_start as usize % core::mem::align_of::<ProgramHeader>() == 0);
            ProgramHeader::from_raw_parts(phdr_start, header.e_phnum.into())
        };
        for ph in phdrs {
            if ph.p_type == PT_LOAD {
                log::debug!("Loading segment");
                let segment = Segment::new(program, ph);
                segment.load(&addrspace, &mut fallocator);
            }
        }

        log::debug!("Setting up stack pages");
        let rsp = untyped_memory_offset;
        for i in 0..stack_pages {
            let frame = fallocator.alloc_user_frame().unwrap().into_raw();
            let addr = rsp - PAGE_SIZE * (i + 1);
            let page = Page::from_start_address(VirtAddr::new(addr));
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
                        PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
                        &mut fallocator,
                    )
                    .unwrap();
            }
        }

        let untyped_memory_pages = untyped_memory_length / PAGE_SIZE;
        log::debug!("Setting up {untyped_memory_pages} untyped memory pages");
        for i in 0..untyped_memory_pages {
            let frame = RawFrame::from_start_address(PhysAddr::new(i as u64 * FRAME_SIZE));
            let page = Page::from_start_address(VirtAddr::new(
                (frame.base().as_u64() + untyped_memory_offset as u64) as usize,
            ));
            // SAFETY: Mapping non-user accessible untyped pages.
            // FIXME: Do this with huge pages? Since each l1 table can hold
            // 256 entries, we are effectively wasting a bit over 1/256 frames
            // in the system.
            unsafe {
                addrspace
                    .map_to(
                        page,
                        frame,
                        PageTableFlags::PRESENT,
                        PageTableFlags::PRESENT,
                        &mut fallocator,
                    )
                    .unwrap();
            }
        }

        log::info!("Initialized user process");
        Ok(Self {
            entry,
            rsp: untyped_memory_offset as u64,
            l4_table,
        })
    }

    pub fn exec(self) -> ! {
        let execution_stack = ExecCtx::new(
            self.l4_table.frame(),
            Regs {
                control: ControlRegs {
                    rsp: self.rsp,
                    rip: self.entry,
                    rflags: 0x202,
                },
                ..Default::default()
            },
        );
        log::debug!("Entering process at {:#X}", self.entry);
        execution_stack.dispatch();
    }
}

struct Segment<'prog, 'head> {
    program: &'prog [u8],
    header: &'head ProgramHeader,
}

impl<'prog, 'head> Segment<'prog, 'head> {
    pub fn new(program: &'prog [u8], header: &'head ProgramHeader) -> Self {
        Self { program, header }
    }

    pub fn load(&self, address_space: &Addrspace, fallocator: &mut BumpAllocator) {
        let vm_range = self.header.p_vaddr..(self.header.p_vaddr + self.header.p_memsz);
        let file_range = self.header.p_offset..(self.header.p_offset + self.header.p_filesz);

        assert!(vm_range.end <= 0xFFFF800000000000);
        assert!(file_range.end <= self.program.len() as u64);
        assert!(self.header.p_memsz >= self.header.p_filesz);
        let mut vcurrent = vm_range.start;
        let mut fcurrent = file_range.start;
        while vcurrent < vm_range.end {
            let frame = fallocator.alloc_user_frame().unwrap().into_raw();
            let page = Page::containing_address(VirtAddr::new(vcurrent as usize));
            let flags = self.header.p_flags;
            let mut pflags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
            assert!(flags & PF_R != 0);
            if flags & PF_W != 0 {
                pflags |= PageTableFlags::WRITABLE;
            }
            if flags & PF_X == 0 {
                pflags |= PageTableFlags::NO_EXECUTE;
            }
            log::info!("Mapping {page:?} to {frame:?} with {pflags:?}");
            // SAFETY: Just mapping the elf data.
            unsafe {
                address_space
                    .map_to(
                        page,
                        frame,
                        pflags,
                        PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
                        fallocator,
                    )
                    .unwrap();
            }

            let offset_page: *mut u8 = frame.base().to_virtual().as_mut_ptr();

            let count = usize::min(
                (file_range.end - fcurrent) as usize,
                PAGE_SIZE - vcurrent as usize % PAGE_SIZE,
            );

            // SAFETY: Hopefully no bugs here...
            unsafe {
                core::ptr::write_bytes(offset_page, 0, PAGE_SIZE);
                if count > 0 {
                    core::ptr::copy(
                        self.program.as_ptr().add(fcurrent as usize),
                        offset_page.add(vcurrent as usize % PAGE_SIZE),
                        count,
                    )
                }
            }
            vcurrent += PAGE_SIZE as u64 - vcurrent % PAGE_SIZE as u64;
            fcurrent += count as u64;
        }
    }
}
