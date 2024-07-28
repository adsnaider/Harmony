//! Boot process initialization

use core::mem::MaybeUninit;
use core::ops::Range;

use loader::{MemFlags, Program, SegmentLoadError, SegmentLoader};

use super::paging::page_table::AnyPageTable;
use crate::arch::exec::{ControlRegs, Regs};
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

pub struct Loader<'a, 'b> {
    address_space: Addrspace<'a>,
    fallocator: &'b mut BumpAllocator,
}

impl SegmentLoader for Loader<'_, '_> {
    fn request_virtual_memory_range(
        &mut self,
        range: Range<usize>,
        rwx: MemFlags,
    ) -> Result<&mut [MaybeUninit<u8>], SegmentLoadError> {
        if range.is_empty() {
            return Ok(&mut []);
        }
        let start = VirtAddr::new(range.start);
        let last_byte = VirtAddr::new(range.end - 1);
        if last_byte.is_higher_half() {
            return Err(SegmentLoadError::InvalidVirtualRange);
        }
        let start_page = Page::containing_address(start).index();
        let end_page = Page::containing_address(last_byte).index();

        let mut pflags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
        if !rwx.readable() {
            return Err(SegmentLoadError::BadMemoryFlags);
        }
        if rwx.writeable() {
            pflags |= PageTableFlags::WRITABLE;
        }
        if !rwx.executable() {
            pflags |= PageTableFlags::NO_EXECUTE;
        }
        let kernel_addrspace = Addrspace::current();
        // Virtual address used for writing into the loaded process.
        const WRITE_START_PAGE: usize = 1;
        let num_pages = end_page - start_page + 1;
        for i in 0..num_pages {
            let page = Page::from_index(start_page + i).unwrap();
            let write_page = Page::from_index(WRITE_START_PAGE + i).unwrap();
            let frame = self.fallocator.alloc_user_frame().unwrap().into_raw();
            log::info!("Mapping {page:?} to {frame:?} with {pflags:?}");
            unsafe {
                let _ = self
                    .address_space
                    .map_to(
                        page,
                        frame,
                        pflags,
                        // Parent flags are the least restrictive since they will be reused for many pages.
                        PageTableFlags::PRESENT
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::USER_ACCESSIBLE,
                        self.fallocator,
                    )
                    .unwrap();

                if let Ok((flusher, _, _)) = kernel_addrspace.unmap(write_page) {
                    flusher.flush();
                }
                kernel_addrspace
                    .map_to(
                        write_page,
                        frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::NO_EXECUTE,
                        PageTableFlags::PRESENT
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::NO_EXECUTE,
                        self.fallocator,
                    )
                    .unwrap()
                    .flush();
            }
        }

        let write_start = unsafe {
            Page::from_index(WRITE_START_PAGE)
                .unwrap()
                .base()
                .as_mut_ptr::<MaybeUninit<u8>>()
                .add(start.as_usize() % PAGE_SIZE)
        };
        let vrange = unsafe { core::slice::from_raw_parts_mut(write_start, range.len()) };
        Ok(vrange)
    }

    unsafe fn release_virtual_memory_range(&mut self, _range: Range<usize>) {
        panic!("Can't release the booter's resources as there's nothing else to be run");
    }
}

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
        let program = Program::new(program).unwrap();

        log::debug!("Setting up process address space");
        let l4_table = {
            let l4_frame = fallocator.alloc_untyped_frame().unwrap();
            AnyPageTable::new_l4(l4_frame).unwrap()
        };
        let addrspace = unsafe { l4_table.as_addrspace() };

        let mut loader = Loader {
            address_space: addrspace,
            fallocator: &mut fallocator,
        };
        let process = program.load(&mut loader).unwrap();
        log::trace!("Entry: {:X}", process.entry);

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
                    .unwrap()
                    .flush();
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
                    .unwrap()
                    .flush();
            }
        }

        log::info!("Initialized user process");
        Ok(Self {
            entry: process.entry,
            rsp: untyped_memory_offset as u64,
            l4_table,
        })
    }

    pub fn into_parts(self) -> (Regs, KPtr<AnyPageTable>) {
        (
            Regs {
                control: ControlRegs {
                    rsp: self.rsp,
                    rip: self.entry,
                    rflags: 0x202,
                },
                ..Default::default()
            },
            self.l4_table,
        )
    }
}
