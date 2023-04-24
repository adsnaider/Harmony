//! Physical frame allocation and management.

use bitalloc::{Bitalloc, Indexable};
use bootloader_api::info::{MemoryRegion, MemoryRegionKind, MemoryRegions};
use critical_section::CriticalSection;
use singleton::Singleton;
use x86_64::structures::paging::{FrameAllocator, PageSize, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

pub(crate) static FRAME_ALLOCATOR: Singleton<SystemFrameAllocator> = Singleton::uninit();

struct Frame(PhysFrame<Size4KiB>);

unsafe impl Indexable for Frame {
    fn index(&self) -> usize {
        (self.0.start_address().as_u64() / Size4KiB::SIZE) as usize
    }

    fn from_index(idx: usize) -> Self {
        // SAFETY: Address will be aligned to Size4KiB::SIZE.
        unsafe {
            Self(PhysFrame::from_start_address_unchecked(PhysAddr::new(
                (idx as u64) * Size4KiB::SIZE,
            )))
        }
    }
}

pub(crate) struct SystemFrameAllocator(Bitalloc<'static, Frame>);

// SAFETY: We use a bitmap to make sure that all frames returned are unique
// and available for use.
unsafe impl FrameAllocator<Size4KiB> for SystemFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.0.allocate().ok().map(|f| f.0)
    }
}

/// Returns true if the memory region is generally usable.
fn is_region_usable(region: &MemoryRegion) -> bool {
    matches!(region.kind, MemoryRegionKind::Usable)
}

pub fn init(pmo: VirtAddr, memory_map: &mut MemoryRegions, cs: CriticalSection) {
    // UEFI makes no guarantees that the memory map is sorted in ascending order so we have to
    // get the last frame by iterating through all of them.
    let nframes = memory_map.iter().fold(0, |last, reg| {
        let last_in_region = reg.end / Size4KiB::SIZE;
        core::cmp::max(last_in_region, last)
    });

    let bytes_required = (nframes - 1) / 8 + 1;
    let frames_required = (bytes_required - 1) / Size4KiB::SIZE + 1;
    log::debug!("Frame allocator requires {bytes_required}B ({frames_required} frames) to function for {nframes} frames");

    // It's much easier to get all of these frames if they are adjacent.
    // Because we still can't allocate, we can't segment the memory map yet, so we instead
    // "remove" these pages from the memory map by chainging the region's start and page_count.
    let available_region = memory_map
        .iter_mut()
        .find(|reg| is_region_usable(reg) && (reg.end - reg.start) >= bytes_required)
        .expect("Couldn't find memory region to setup frame allocation");
    assert!((available_region.end - available_region.start) / Size4KiB::SIZE >= frames_required);
    log::debug!(
        "Found available storage for frame allocator at physical address {:#?}",
        available_region.start as *const ()
    );

    // SAFETY: Memory map comes from the bootloader. We update the missing entries in the map
    // such that the frame allocator doesn't allocate itself. This is provided by the
    // `Bitalloc::new_with_availability` function that takes the iterator of the available frames.
    unsafe {
        let storage = core::slice::from_raw_parts_mut(
            (pmo + available_region.start as u64).as_ptr::<u64>() as *mut u64,
            (bytes_required as usize - 1) / 8 + 1,
        );

        available_region.start += frames_required * Size4KiB::SIZE;

        let (bitalloc, _leftover) = Bitalloc::new_available(
            storage,
            nframes as usize,
            memory_map
                .iter()
                .filter(|reg| is_region_usable(reg))
                .flat_map(|reg| {
                    let count = (reg.end - reg.start) / Size4KiB::SIZE;
                    (0..count).map(|i| {
                        Frame(PhysFrame::from_start_address_unchecked(
                            PhysAddr::new_unsafe(reg.start + i * Size4KiB::SIZE),
                        ))
                    })
                }),
        );
        FRAME_ALLOCATOR.initialize(SystemFrameAllocator(bitalloc), cs);
    }
}
