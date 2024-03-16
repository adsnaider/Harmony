//! Utilities used for the intialization sequence to bootstrap the init process

use goblin::elf::program_header::{PF_R, PF_W, PF_X, PT_LOAD};
use goblin::elf64::header::{Header, SIZEOF_EHDR};
use goblin::elf64::program_header::ProgramHeader;
use x86_64_impl::registers::control::{Cr3, Cr3Flags};
use x86_64_impl::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, Size4KiB,
};
use x86_64_impl::VirtAddr;

use super::paging::RawFrame;
use crate::arch::sysret;
use crate::arch::x86_64::gdt::PRIVILEGE_STACK_ADDR;
use crate::util::FrameBumpAllocator;
use crate::PMO;

pub struct Process {
    entry: u64,
    l4_table: RawFrame,
}

unsafe impl FrameAllocator<Size4KiB> for FrameBumpAllocator {
    fn allocate_frame(&mut self) -> Option<x86_64_impl::structures::paging::PhysFrame<Size4KiB>> {
        self.alloc_frame()
            .map(|frame| frame.into_kernel().into_raw().into_phys_frame())
    }
}

#[derive(Debug)]
pub enum LoadError {}

fn new_l4_table<'a>(frame: RawFrame) -> &'a mut PageTable {
    let current: &PageTable = unsafe {
        let frame = RawFrame::from_start_address(Cr3::read().0.start_address().as_u64());
        &*frame.as_ptr()
    };

    let new: &mut PageTable = unsafe {
        core::ptr::write(frame.as_ptr_mut(), current.clone());
        &mut *frame.as_ptr_mut()
    };
    for entry in new.iter_mut().take(256) {
        entry.set_unused();
    }
    new
}

impl Process {
    pub fn load(program: &[u8], fallocator: &mut FrameBumpAllocator) -> Result<Self, LoadError> {
        assert!(
            program.as_ptr() as usize % 16 == 0,
            "ELF must be aligned to 16 bytes"
        );

        let l4_frame = fallocator.alloc_frame().unwrap().into_kernel().into_raw();
        let mut addrspace = unsafe {
            let l4_table = new_l4_table(l4_frame.clone());
            OffsetPageTable::new(l4_table, VirtAddr::new(*PMO as u64))
        };
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
        const STACK_PAGES: usize = 1;

        for i in 0..STACK_PAGES {
            let frame = fallocator
                .alloc_frame()
                .unwrap()
                .into_user()
                .into_raw()
                .into_phys_frame();
            let addr = rsp - 4096 * (i + 1);
            let page = Page::from_start_address(VirtAddr::new(addr as u64)).unwrap();
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
                    .flush();
            }
        }

        let interrupt_stack = fallocator
            .alloc_frame()
            .unwrap()
            .into_kernel()
            .into_raw()
            .into_phys_frame();
        let interrupt_stack_page =
            Page::from_start_address(VirtAddr::new(PRIVILEGE_STACK_ADDR)).unwrap();
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
                .unwrap()
                .flush();
        }
        log::debug!("Mapped interrupt stack");
        Ok(Self {
            entry,
            l4_table: l4_frame,
        })
    }

    pub fn exec(self) -> ! {
        log::debug!("Entering process at {:#X}", self.entry);
        // SAFETY: The entry and rsp are valid for the user process.
        unsafe {
            Cr3::write(
                self.l4_table.into_phys_frame(),
                Cr3Flags::PAGE_LEVEL_WRITETHROUGH,
            );
            sysret(self.entry, 0x0000_8000_0000_0000);
        }
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

    pub fn load(
        &self,
        address_space: &mut OffsetPageTable<'_>,
        fallocator: &mut FrameBumpAllocator,
    ) {
        let vm_range = self.header.p_vaddr..(self.header.p_vaddr + self.header.p_memsz);
        let file_range = self.header.p_offset..(self.header.p_offset + self.header.p_filesz);

        assert!(vm_range.end <= 0xFFFF800000000000);
        assert!(file_range.end <= self.program.len() as u64);
        assert!(self.header.p_memsz >= self.header.p_filesz);
        let mut vcurrent = vm_range.start;
        let mut fcurrent = file_range.start;
        while vcurrent < vm_range.end {
            let frame = fallocator.alloc_frame().unwrap().into_user().into_raw();
            let page = Page::containing_address(VirtAddr::new(vcurrent));
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
                    .map_to(page, frame.clone().into_phys_frame(), pflags, fallocator)
                    .unwrap()
                    .flush();
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
