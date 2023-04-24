use core::alloc::{AllocError, Allocator, GlobalAlloc, Layout};
use core::ptr::NonNull;

use critical_section::CriticalSection;
use linked_list_allocator::Heap;
use singleton::Singleton;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB, Translate,
};
use x86_64::VirtAddr;

use super::frames::FRAME_ALLOCATOR;
use super::paging::PAGE_MAPPER;

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

pub fn init<M, F>(page_map: &mut M, frame_allocator: &mut F, cs: CriticalSection)
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
