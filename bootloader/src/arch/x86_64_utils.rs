//! x86_64 utilities.
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size1GiB,
    Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

use crate::PAGING_MEMORY;

/// Allocator that can be used with a page table implementation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PageFrameAllocator {}

unsafe impl FrameAllocator<Size4KiB> for PageFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let Ok(buffer) = (unsafe {crate::sys::alloc::get_pages(None, 1, PAGING_MEMORY)}) else {
            return None;
        };
        let frame = PhysFrame::from_start_address(PhysAddr::new(buffer.as_ptr() as u64)).unwrap();
        Some(frame)
    }
}

/// Maps all physical memory as described in the memory map to the virtual space offseted by
/// `physical_memory_offset` while also maintining the identity mapping that is expected by UEFI.
///
/// # Returns
///
/// The page table resulting from this mapping.
///
/// # Safety
///
/// This will overwrite all virtual pages at
/// [physical_memory_offset, physical_memory_offset + last page]
///
/// The lifetime of the page tables is bounded to the lifetime of the physical frames used
/// internally. These will be tagged in the memory map.
pub unsafe fn remap_memory_to_offset<'a>(physical_memory_offset: usize) -> OffsetPageTable<'a> {
    let physical_memory_offset = VirtAddr::new(physical_memory_offset as u64);
    assert!(physical_memory_offset.is_aligned(4096u64));
    // Frame allocator that allocates pages of type `PAGING_MEMORY`.
    let mut fallocator = PageFrameAllocator {};

    // We need the memory map to figure out how large our physical address space is.
    let memory_map = crate::sys::get_memory_map();

    let (inactive_l4_table, l4_frame) = {
        let frame = fallocator.allocate_frame().unwrap();
        let table = frame.start_address().as_u64() as *mut PageTable;
        // SAFETY: alignment is appropriate as a PageTable is aligned to frame boundary.
        unsafe {
            // emtpy l4 table.
            table.write(PageTable::new());
            (&mut *table, frame)
        }
    };
    // SAFETY: UEFI is identity mapped. L4 table is valid (as initialized above) though not active yet.
    let mut page_table =
        unsafe { OffsetPageTable::new(inactive_l4_table, VirtAddr::new_unsafe(0)) };

    // Get the latest page address. UEFI doesn't really guarantee that this will be the last descriptor.
    let latest_page_addr = memory_map.fold(0, |max, region| {
        core::cmp::max(max, region.phys_start + 4096 * (region.page_count - 1))
    });

    // Remap memory in 1GB pages.
    for phys_start in (0..=latest_page_addr).step_by(1024 * 1024 * 1024) {
        let frame: PhysFrame<Size1GiB> =
            PhysFrame::from_start_address(PhysAddr::new(phys_start)).unwrap();
        // SAFETY: We aren't actually performing any maps here as the table is
        // currently inactive. However, the identity mappings should be the same as those
        // in UEFI and the remapped addresses are part of the precondition of the function.
        unsafe {
            log::info!("Identity mapping: {frame:?}");
            page_table
                .identity_map(
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    &mut fallocator,
                )
                .expect("Couldn't identity map {frame:?}")
                .ignore();

            let mapped_page: Page<Size1GiB> =
                Page::from_start_address_unchecked(physical_memory_offset + phys_start);
            log::info!("Remapping {frame:?} to {mapped_page:?}");
            page_table
                .map_to(
                    mapped_page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    &mut fallocator,
                )
                .unwrap()
                .ignore();
        }
    }

    // SAFETY: The l4 table is correct and shouldn't break original memory expectations.
    unsafe {
        let (_, cr3_flags) = Cr3::read();
        Cr3::write(l4_frame, cr3_flags);
    }
    page_table
}
