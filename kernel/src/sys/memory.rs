//! System memory utilities.

use core::alloc::{AllocError, Allocator, GlobalAlloc, Layout};
use core::ptr::NonNull;

use bitalloc::{Bitalloc, Indexable};
use bootloader_api::info::{MemoryRegion, MemoryRegionKind, MemoryRegions};
use critical_section::CriticalSection;
use linked_list_allocator::Heap;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags, PhysFrame,
    Size4KiB, Translate,
};
use x86_64::{PhysAddr, VirtAddr};

use crate::singleton::Singleton;

struct Frame(PhysFrame<Size4KiB>);

// SAFETY: Each frame is uniquely indexed by it's starting address, normalized.
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

struct SystemFrameAllocator(Bitalloc<'static, Frame>);

// SAFETY: We use a bitmap to make sure that all frames returned are unique
// and available for use.
unsafe impl FrameAllocator<Size4KiB> for SystemFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.0.allocate().ok().map(|f| f.0)
    }
}

static PAGE_MAPPER: Singleton<OffsetPageTable<'static>> = Singleton::uninit();
static FRAME_ALLOCATOR: Singleton<SystemFrameAllocator> = Singleton::uninit();
static MEMORY_ALLOCATOR: Singleton<Heap> = Singleton::uninit();

#[allow(clippy::undocumented_unsafe_blocks)]
// SAFETY: Address is well-aligned and canonical.
const HEAP_START: VirtAddr = unsafe { VirtAddr::new_unsafe(0xFFFF_9000_0000_0000) };

#[allow(clippy::undocumented_unsafe_blocks)]
// SAFETY: Address is well-aligned and canonical.
const HEAP_MAX: VirtAddr = unsafe { VirtAddr::new_unsafe(0xFFFF_A000_0000_0000) };

#[derive(Debug, Copy, Clone)]
struct MemoryManager {}

#[global_allocator]
static GLOBAL_ALLOCATOR: MemoryManager = MemoryManager {};

/// Returns true if the memory region is generally usable.
fn is_region_usable(region: &MemoryRegion) -> bool {
    matches!(region.kind, MemoryRegionKind::Usable)
}

/// Initializes the system memory management. Allocation calls can be made after `init`.
///
/// # Safety
///
/// The physical_memory_offset must be in accordance to the currently setup page table and the
/// memory map must represent memory accurately.
///
/// # Panics
///
/// If called more than once.
pub unsafe fn init(physical_memory_offset: VirtAddr, memory_map: &mut MemoryRegions) {
    critical_section::with(|cs| {
        init_frame_allocator(physical_memory_offset, memory_map, cs);
        log::info!("Initialized frame allocator.");

        init_page_map(physical_memory_offset, cs);
        log::info!("Page mapper initialized");

        init_allocator(
            &mut *PAGE_MAPPER.lock(cs),
            &mut *FRAME_ALLOCATOR.lock(cs),
            cs,
        );
        log::info!("Allocator initialized");
    })
}

// SAFETY: We implement the allocator the linked list allocator and the frame allocator
// to map pages as needed.
unsafe impl Allocator for MemoryManager {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        critical_section::with(|cs| {
            let mut allocator = MEMORY_ALLOCATOR.lock(cs);
            loop {
                match allocator.allocate_first_fit(layout) {
                    // SAFETY: `ptr` should not be null and size should be at least `layout.size()`.
                    Ok(ptr) => unsafe {
                        return Ok(NonNull::new_unchecked(core::ptr::slice_from_raw_parts_mut(
                            ptr.as_ptr(),
                            layout.size(),
                        )));
                    },
                    Err(_) => {
                        let mut frame_allocator = FRAME_ALLOCATOR.lock(cs);
                        let mut page_mapper = PAGE_MAPPER.lock(cs);
                        let frame = frame_allocator.allocate_frame().ok_or(AllocError {})?;
                        // SAFETY: The heap page is well aligned since we always allocate multiple of page
                        // sizes to extend the allocator.
                        let next_heap_page = unsafe {
                            Page::from_start_address_unchecked(VirtAddr::new_unsafe(
                                allocator.top() as u64,
                            ))
                        };

                        if next_heap_page.start_address() >= HEAP_MAX {
                            return Err(AllocError);
                        }

                        // SAFETY: We artificially set the limits of the virtual memory to prevent
                        // virtual memory collisions and the physical frame has just been allocated.
                        // See `virtual_memory_segmentation.md` for more information.
                        unsafe {
                            page_mapper
                                .map_to(
                                    next_heap_page,
                                    frame,
                                    PageTableFlags::PRESENT
                                        | PageTableFlags::WRITABLE
                                        | PageTableFlags::NO_EXECUTE,
                                    &mut *frame_allocator,
                                )
                                .or(Err(AllocError))?
                                .flush();

                            allocator.extend(4096);
                        }
                    }
                }
            }
        })
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: Function precondition.
        critical_section::with(|cs| unsafe { MEMORY_ALLOCATOR.lock(cs).deallocate(ptr, layout) })
    }
}

// SAFETY: Just a thin wrapper over `Allocator` impl.
unsafe impl GlobalAlloc for MemoryManager {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match self.allocate(layout) {
            Ok(ptr) => ptr.as_ptr() as *mut u8,
            Err(_) => core::ptr::null_mut(),
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: Function precondition.
        unsafe {
            self.deallocate(NonNull::new_unchecked(ptr), layout);
        }
    }
}

fn init_frame_allocator(pmo: VirtAddr, memory_map: &mut MemoryRegions, cs: CriticalSection) {
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

fn init_page_map(pmo: VirtAddr, cs: CriticalSection) {
    let l4_table = {
        let (frame, _) = x86_64::registers::control::Cr3::read();
        let virt = pmo + frame.start_address().as_u64();
        // SAFETY: This is valid since the PageTable is initialized in the cr3 and the physical
        // memory offset must be correct.
        unsafe { &mut *(virt.as_u64() as *mut PageTable) }
    };

    // SAFETY: We get the l4_table provided by the bootloader which maps the memory to
    // `pmo`.
    let page_map = unsafe { OffsetPageTable::new(l4_table, pmo) };

    // Sanity check, let's check some small addresses, should be mapped to themselves.
    assert!(page_map.translate_addr(pmo + 0x0u64) == Some(PhysAddr::new(0)));
    assert!(page_map.translate_addr(pmo + 0xABCDu64) == Some(PhysAddr::new(0xABCD)));
    assert!(page_map.translate_addr(pmo + 0xABAB_0000u64) == Some(PhysAddr::new(0xABAB_0000)));

    PAGE_MAPPER.initialize(page_map, cs);
}

fn init_allocator<M, F>(page_map: &mut M, frame_allocator: &mut F, cs: CriticalSection)
where
    M: Mapper<Size4KiB> + Translate,
    F: FrameAllocator<Size4KiB>,
{
    // SAFETY: Address is aligned to page boundary and is canonical.
    let heap_page: Page<Size4KiB> = unsafe { Page::from_start_address_unchecked(HEAP_START) };

    // SAFETY: Heap page isn't originally mapped to anything and frame is recently allocated.
    unsafe {
        let frame = frame_allocator.allocate_frame().expect("Out of frames");

        page_map
            .map_to(
                heap_page,
                frame,
                PageTableFlags::WRITABLE | PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                frame_allocator,
            )
            .expect("Error mapping an initial heap page.")
            .flush();

        assert_eq!(
            page_map.translate_addr(heap_page.start_address()).unwrap(),
            frame.start_address()
        );
    }

    // SAFETY: The memory has been allocated and will only be used by the allocator.
    let allocator = unsafe { Heap::new(HEAP_START.as_mut_ptr(), 4096) };
    MEMORY_ALLOCATOR.initialize(allocator, cs);
}
