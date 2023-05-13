//! Userspace (ring 3) context.
use core::arch::asm;

use goblin::elf::{Elf, ProgramHeader};
use goblin::elf64::program_header::PT_LOAD;
use thiserror::Error;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageSize, PageTableFlags, PhysFrame, Size4KiB,
};
use x86_64::VirtAddr;

use super::Context;
use crate::arch::mm::{self, FRAME_ALLOCATOR};

/// A runnable context that can be scheduled.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct UserContext {
    l4_frame: PhysFrame,
    regs: CpuRegs,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
struct CpuRegs {
    rip: u64,
    rsp: u64,
}

/// Error loading the binary
#[derive(Debug, Error)]
pub enum LoadError {
    /// Error parsing the binary as an ELF64 executable
    #[error("Error parsing the binary")]
    Parse(goblin::error::Error),
    /// Error loading one of the segments
    #[error("Error loading a segment")]
    SegmentLoad(#[from] SegmentLoadError),
    /// The system is out of physical frames
    #[error("System out of frames")]
    OutOfFrames,
}

impl From<goblin::error::Error> for LoadError {
    fn from(value: goblin::error::Error) -> Self {
        Self::Parse(value)
    }
}

struct Segment<'a> {
    program: &'a [u8],
    header: &'a ProgramHeader,
}

/// Error loading the binary.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Error)]
pub enum SegmentLoadError {
    /// Segment overlaps with the kernel.
    #[error("The virtual address space of this segment overlaps with the kernel")]
    KernelOverlap,
    /// The segment isn't aligned to 0x1000
    #[error("The segment isn't aligned to a page boundary")]
    SegmentUnaligned,
    /// The segment requested goes past the end of the program bytes
    #[error("The segment references a location in the file past its end")]
    BufferOverflow,
    /// The number of bytes requested from the program exceeds the number of bytes to place in memory
    #[error("The requested bytes from the file exceeds that of the memory")]
    SizeError,
}

impl<'a> Segment<'a> {
    pub fn new(program: &'a [u8], header: &'a ProgramHeader) -> Self {
        assert!(header.p_type == PT_LOAD);
        Self { program, header }
    }

    pub fn load<F>(
        &self,
        frame_allocator: &mut F,
        virtual_mapping: &mut OffsetPageTable,
    ) -> Result<(), SegmentLoadError>
    where
        F: FrameAllocator<Size4KiB>,
    {
        let vm_range = self.header.vm_range();
        let file_range = self.header.file_range();

        if vm_range.end > 0xFFFF_8000_0000_0000 {
            return Err(SegmentLoadError::KernelOverlap);
        }
        if vm_range.start % 4096 != 0 {
            return Err(SegmentLoadError::SegmentUnaligned);
        }
        if file_range.end > self.program.len() {
            return Err(SegmentLoadError::BufferOverflow);
        }
        if self.header.p_filesz > self.header.p_memsz {
            return Err(SegmentLoadError::SizeError);
        }

        let frames_needed = (self.header.p_memsz - 1) / 4096 + 1;
        let mut remaining = self.header.p_filesz as usize;
        for i in 0..frames_needed {
            let frame = frame_allocator.allocate_frame().unwrap();
            let page =
                Page::from_start_address(VirtAddr::new(vm_range.start as u64 + i * 4096)).unwrap();
            let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
            if self.header.is_write() {
                flags |= PageTableFlags::WRITABLE;
            }
            if !self.header.is_executable() {
                flags |= PageTableFlags::NO_EXECUTE;
            }
            // TODO: WRITABLE.
            unsafe {
                virtual_mapping
                    .map_to(page, frame, flags, frame_allocator)
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
                        .add((self.header.p_offset + i * 4096) as usize),
                    offset_page,
                    usize::min(remaining, 4096),
                )
            }
            remaining -= usize::min(remaining, 4096);
        }
        Ok(())
    }
}

impl UserContext {
    /// Loads the program into memory.
    pub fn load(program: &[u8]) -> Result<Self, LoadError> {
        let elf = Elf::parse(program)?;
        critical_section::with(|cs| {
            let (mut page_map, l4_frame) =
                unsafe { mm::make_new_page_table().ok_or(LoadError::OutOfFrames)? };
            let mut frame_allocator = FRAME_ALLOCATOR.lock(cs);
            for ph in elf.program_headers.iter().filter(|ph| ph.p_type == PT_LOAD) {
                let segment = Segment::new(program, ph);
                segment.load(&mut *frame_allocator, &mut page_map)?;
            }

            const STACK_TOP: u64 = 0x0000_8000_0000_0000;
            let stack_frame = frame_allocator
                .allocate_frame()
                .ok_or(LoadError::OutOfFrames)?;
            let stack_page =
                Page::from_start_address(VirtAddr::new(STACK_TOP - Size4KiB::SIZE)).unwrap();

            unsafe {
                page_map
                    .map_to(
                        stack_page,
                        stack_frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::USER_ACCESSIBLE
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::NO_EXECUTE,
                        &mut *frame_allocator,
                    )
                    .unwrap()
                    .ignore();
            }

            Ok(Self {
                l4_frame,
                regs: CpuRegs {
                    rip: elf.entry,
                    rsp: STACK_TOP,
                },
            })
        })
    }
}

impl Context for UserContext {
    fn switch(&self) -> ! {
        let (_, flags) = Cr3::read();
        unsafe {
            Cr3::write(self.l4_frame, flags);
            sysret(&self.regs);
        }
    }

    fn completed(&self) -> bool {
        todo!();
    }
}

unsafe fn sysret(regs: &CpuRegs) -> ! {
    unsafe {
        asm!(
            "mov rsp, {stack}",
            "sysretq",
            stack =in(reg) regs.rsp,
            in("r11") 0x202,
            in("rcx") regs.rip,
            options(noreturn)
        );
    }
}
