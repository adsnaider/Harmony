use core::arch::asm;

use goblin::elf::program_header::PT_LOAD;
use goblin::elf::{Elf, ProgramHeader};
use thiserror::Error;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags, PhysFrame,
};
use x86_64::VirtAddr;

use crate::sys::memory::{self, SystemFrameAllocator, FRAME_ALLOCATOR};

/// Context associated with a process.
#[derive(Debug)]
pub struct Process {
    page_table: OffsetPageTable<'static>,
    l4_frame: PhysFrame,
    regs: CpuRegs,
}

#[derive(Debug)]
struct CpuRegs {
    rip: u64,
    rsp: u64,
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("Error parsing the binary")]
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

    pub fn load(
        &self,
        frame_allocator: &mut SystemFrameAllocator,
        virtual_mapping: &mut OffsetPageTable,
    ) {
        let vm_range = self.virt_addr..(self.virt_addr + self.memsz);
        let file_range = self.file_off..(self.file_off + self.filesz);

        assert!(vm_range.end <= 0xFFFF800000000000);
        assert!(vm_range.start % 4096 == 0);
        assert!(file_range.end <= self.program.len() as u64);
        assert!(self.memsz >= self.filesz);
        let frames_needed = (self.memsz - 1) / 4096 + 1;
        let mut remaining = self.filesz as usize;
        for i in 0..frames_needed {
            let frame = frame_allocator.allocate_frame().unwrap();
            let page = Page::from_start_address(VirtAddr::new(vm_range.start + i * 4096)).unwrap();
            unsafe {
                virtual_mapping
                    .map_to(
                        page,
                        frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::USER_ACCESSIBLE
                            | PageTableFlags::WRITABLE,
                        frame_allocator,
                    )
                    .unwrap()
                    .ignore();
            }

            let offset_page = (frame.start_address().as_u64()
                + virtual_mapping.phys_offset().as_u64()) as *mut u8;
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

pub fn init() {
    sce_enable();
}

fn sce_enable() {
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

impl Process {
    /// Loads the process to main memory and returns a context associated with it.
    pub fn load(program: &[u8], stack_pages: u64) -> Result<Self, goblin::error::Error> {
        let elf = Elf::parse(program)?;
        assert!(elf.is_64);
        let (mut page_table, l4_frame) = unsafe { memory::shallow_clone_page_table() };
        critical_section::with(|cs| {
            let mut frame_allocator = FRAME_ALLOCATOR.lock(cs);
            for ph in elf.program_headers.iter().filter(|ph| ph.p_type == PT_LOAD) {
                let segment = Segment::new(program, ph);
                segment.load(&mut frame_allocator, &mut page_table);
            }
        });
        let rsp = 0x0000_8000_0000_0000;

        critical_section::with(|cs| {
            let mut frame_allocator = FRAME_ALLOCATOR.lock(cs);
            for i in 0..stack_pages {
                let frame = frame_allocator.allocate_frame().unwrap();
                let addr = rsp - 4096 * (i + 1);
                let page = Page::from_start_address(VirtAddr::new(addr)).unwrap();
                log::debug!("Mapping page {page:?} to frame {frame:?}");
                unsafe {
                    page_table
                        .map_to(
                            page,
                            frame,
                            PageTableFlags::PRESENT
                                | PageTableFlags::WRITABLE
                                | PageTableFlags::USER_ACCESSIBLE
                                | PageTableFlags::NO_EXECUTE,
                            &mut *frame_allocator,
                        )
                        .unwrap()
                        .ignore();
                }
            }
        });
        Ok(Self {
            page_table,
            l4_frame,
            regs: CpuRegs {
                rip: elf.entry,
                rsp,
            },
        })
    }

    /// Runs the user process.
    pub unsafe fn exec(&self) -> ! {
        let (_, flags) = Cr3::read();
        unsafe {
            Cr3::write(self.l4_frame, flags);
            crate::sys::gdt::sysret(self.regs.rip, self.regs.rsp);
        }
    }
}
