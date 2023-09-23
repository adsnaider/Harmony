use core::alloc::{AllocError, Allocator, GlobalAlloc, Layout};
use core::ptr::NonNull;

use critical_section::CriticalSection;
use linked_list_allocator::Heap;
use singleton::Singleton;

use super::VirtPage;
use crate::arch::mm::paging::{AddrSpace, PageTableFlags};
use crate::arch::mm::Frame;

static MEMORY_ALLOCATOR: Singleton<Heap> = Singleton::uninit();

#[allow(clippy::undocumented_unsafe_blocks)]
// SAFETY: Address is well-aligned and canonical.
const HEAP_START: u64 = 0xFFFF_9000_0000_0000;

#[allow(clippy::undocumented_unsafe_blocks)]
// SAFETY: Address is well-aligned and canonical.
const HEAP_MAX: u64 = 0xFFFF_A000_0000_0000;

#[derive(Debug, Copy, Clone)]
struct MemoryManager {}

#[global_allocator]
static GLOBAL_ALLOCATOR: MemoryManager = MemoryManager {};

// SAFETY: We implement the allocator the linked list allocator and the frame allocator
// to map pages as needed.
unsafe impl Allocator for MemoryManager {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        log::debug!("malloc request: {layout:?}");
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
                        let frame = Frame::alloc().ok_or(AllocError {})?;
                        // SAFETY: The heap page is well aligned since we always allocate multiple of page
                        // sizes to extend the allocator.
                        let next_heap_page =
                            VirtPage::from_start_address(allocator.top() as u64).unwrap();

                        if next_heap_page.start_address() >= HEAP_MAX {
                            return Err(AllocError);
                        }

                        // SAFETY: We artificially set the limits of the virtual memory to prevent
                        // virtual memory collisions and the physical frame has just been allocated.
                        // See `virtual_memory_segmentation.md` for more information.
                        unsafe {
                            AddrSpace::current()
                                .map_to(
                                    next_heap_page,
                                    frame,
                                    PageTableFlags::PRESENT
                                        | PageTableFlags::WRITABLE
                                        | PageTableFlags::NO_EXECUTE,
                                )
                                .or(Err(AllocError {}))?;
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

pub fn init(cs: CriticalSection<'_>) {
    // SAFETY: Address is aligned to page boundary and is canonical.
    let heap_page = VirtPage::from_start_address(HEAP_START).unwrap();

    // SAFETY: Heap page isn't originally mapped to anything and frame is recently allocated.
    unsafe {
        let frame = Frame::alloc().expect("Out of frames");

        AddrSpace::current()
            .map_to(
                heap_page,
                frame,
                PageTableFlags::WRITABLE | PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
            )
            .expect("Error mapping an initial heap page.");

        assert_eq!(
            AddrSpace::current()
                .translate(heap_page.start_address())
                .unwrap()
                .unwrap(),
            frame.start_address()
        );
    }

    // SAFETY: The memory has been allocated and will only be used by the allocator.
    let allocator = unsafe { Heap::new(HEAP_START as *mut u8, 4096) };
    MEMORY_ALLOCATOR.initialize(allocator, cs);
}
