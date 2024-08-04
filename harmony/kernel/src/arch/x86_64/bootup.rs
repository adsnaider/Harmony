//! Boot process initialization

use core::ops::Range;

use loader::{Loader, MemFlags, Program};

use super::exec::ScratchRegs;
use super::paging::page_table::AnyPageTable;
use super::paging::RawFrame;
use crate::arch::exec::{ControlRegs, Regs};
use crate::arch::paging::page_table::{Addrspace, PageTableFlags};
use crate::arch::paging::{Page, VirtAddr, PAGE_SIZE};
use crate::bump_allocator::BumpAllocator;
use crate::kptr::KPtr;

pub struct Process {
    pub entry: u64,
    pub rsp: u64,
    pub l4_table: KPtr<AnyPageTable>,
    pub initrd: (*const u8, usize),
}

pub struct BootstrapLoader<'a, 'b> {
    address_space: Addrspace<'a>,
    fallocator: &'b mut BumpAllocator,
}

#[derive(Debug, Clone)]
pub enum LoadError {
    InvalidVirtualRange,
    BadMemoryFlags,
    FileRangeLargerThanVirtualRange,
}

impl BootstrapLoader<'_, '_> {
    fn request_page(&mut self, page: Page, rwx: MemFlags) -> Result<&mut [u8], LoadError> {
        let pflags = if rwx.is_empty() {
            PageTableFlags::PRESENT
        } else {
            let mut pflags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
            if !rwx.readable() {
                return Err(LoadError::BadMemoryFlags);
            }
            if rwx.writeable() {
                pflags |= PageTableFlags::WRITABLE;
            }
            if !rwx.executable() {
                pflags |= PageTableFlags::NO_EXECUTE;
            }
            pflags
        };
        let frame = self.fallocator.alloc_user_frame().unwrap().into_raw();
        log::trace!("Mapping {page:?} to {frame:?} with {pflags:?}");
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
        }
        Ok(unsafe {
            core::slice::from_raw_parts_mut(frame.base().to_virtual().as_mut_ptr(), Page::size())
        })
    }

    fn map_page(
        &mut self,
        page: Page,
        frame: RawFrame,
        flags: PageTableFlags,
    ) -> Result<(), LoadError> {
        unsafe {
            let _ = self
                .address_space
                .map_to(
                    page,
                    frame,
                    flags,
                    // Parent flags are the least restrictive since they will be reused for many pages.
                    PageTableFlags::PRESENT
                        | PageTableFlags::WRITABLE
                        | PageTableFlags::USER_ACCESSIBLE,
                    self.fallocator,
                )
                .unwrap();
        }
        Ok(())
    }
}

impl Loader for BootstrapLoader<'_, '_> {
    type Error = LoadError;
    fn load_source(
        &mut self,
        at: Range<usize>,
        mut source: &[u8],
        rwx: MemFlags,
    ) -> Result<(), Self::Error> {
        log::info!("Loading source at {at:X?}");
        if source.len() > at.len() {
            return Err(LoadError::FileRangeLargerThanVirtualRange);
        }
        let mut vaddr = at.start;
        let start_page = at.start / Page::size();
        let end_page = at.end.div_ceil(Page::size());
        for page in start_page..end_page {
            let page = Page::from_index(page).unwrap();
            let dest = self.request_page(page, rwx)?;
            dest.fill(0);

            let dest_start = vaddr % Page::size();
            let dest_end = Page::size();

            let source_start = 0;
            let source_end = source.len();

            let count = usize::min(dest_end - dest_start, source_end - source_start);
            dest[dest_start..(dest_start + count)]
                .copy_from_slice(&source[source_start..(source_start + count)]);
            source = &source[(source_start + count)..];
            vaddr += count;
        }
        Ok(())
    }

    fn load_zeroed(&mut self, at: Range<usize>, rwx: MemFlags) -> Result<(), Self::Error> {
        let start_page = at.start / Page::size();
        let end_page = at.end.div_ceil(Page::size());
        for page in start_page..end_page {
            let page = Page::from_index(page).unwrap();
            let dest = self.request_page(page, rwx)?;
            dest.fill(0);
        }
        Ok(())
    }

    fn load_uninit(&mut self, at: Range<usize>, rwx: MemFlags) -> Result<(), Self::Error> {
        let start_page = at.start / Page::size();
        let end_page = at.end.div_ceil(Page::size());
        for page in start_page..end_page {
            let page = Page::from_index(page).unwrap();
            let _ = self.request_page(page, rwx);
        }
        Ok(())
    }

    unsafe fn unload(&mut self, _vrange: Range<usize>) {
        unimplemented!()
    }
}

impl Process {
    pub fn load(
        program: &[u8],
        stack_pages: usize,
        untyped_memory_offset: usize,
        untyped_memory_length: usize,
        initrd: &[u8],
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

        let mut loader = BootstrapLoader {
            address_space: addrspace,
            fallocator: &mut fallocator,
        };
        log::info!("Loading process headers");
        let process = program.load(&mut loader).unwrap();
        log::debug!("Entry: {:X}", process.entry());

        let stack_top = untyped_memory_offset;
        let stack_bottom = untyped_memory_offset
            .checked_sub(stack_pages * PAGE_SIZE)
            .unwrap();
        log::info!("Setting up stack pages at {:X?}", stack_bottom..stack_top);
        let _ = loader
            .load_zeroed(stack_bottom..stack_top, MemFlags::READ | MemFlags::WRITE)
            .unwrap();

        let untyped_pages = untyped_memory_length / Page::size();
        let untyped_start = Page::from_start_address(VirtAddr::new(untyped_memory_offset)).index();
        log::info!(
            "Setting up {} untyped pages at {:X?}",
            untyped_pages,
            untyped_memory_offset
        );
        for (frame, page) in (untyped_start..(untyped_start + untyped_pages)).enumerate() {
            let page = Page::from_index(page).unwrap();
            let frame = RawFrame::from_index(frame as u64).unwrap();
            loader
                .map_page(page, frame, PageTableFlags::PRESENT)
                .unwrap();
        }
        // And also pass through the initrd image
        let initrd_start = process.top_of_text().div_ceil(Page::size()) * Page::size();
        let initrd_end = initrd_start + initrd.len();
        loader
            .load_source(initrd_start..initrd_end, initrd, MemFlags::READ)
            .unwrap();

        log::info!("Initialized user process");
        Ok(Self {
            entry: process.entry(),
            rsp: untyped_memory_offset as u64,
            l4_table,
            initrd: (initrd_start as *const u8, initrd.len()),
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
                scratch: ScratchRegs {
                    rdi: 0,
                    rsi: self.initrd.0 as usize as u64,
                    rdx: self.initrd.1 as u64,
                    ..Default::default()
                },
                ..Default::default()
            },
            self.l4_table,
        )
    }
}
