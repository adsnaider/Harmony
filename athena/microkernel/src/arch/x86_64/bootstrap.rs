//! Utilities used for the intialization sequence to bootstrap the init process

use x86_64_impl::registers::control::{Cr3, Cr3Flags};
use x86_64_impl::structures::paging::{FrameAllocator, OffsetPageTable, Size4KiB};

use super::paging::RawFrame;
use crate::arch::sysret;
use crate::util::FrameBumpAllocator;

pub struct Process {
    entry: u64,
    l4_table: RawFrame,
}

unsafe impl FrameAllocator<Size4KiB> for FrameBumpAllocator {
    fn allocate_frame(&mut self) -> Option<x86_64_impl::structures::paging::PhysFrame<Size4KiB>> {
        self.alloc_frame().map(|frame| frame.into_phys_frame())
    }
}

#[derive(Debug)]
pub enum LoadError {}

impl Process {
    pub fn load(proc: &[u8], allocator: &mut FrameBumpAllocator) -> Result<Self, LoadError> {
        assert!(
            proc.as_ptr() as usize % 16 == 0,
            "ELF must be aligned to 16 bytes"
        );

        let mut addrspace = OffsetPageTable::new(fallocator.alloc_frame().unwrap());
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
