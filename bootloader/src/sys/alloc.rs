//! UEFI allocation services.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

use bootinfo::UEFI_MEMORY_TYPE;
use uefi::table::boot::MemoryType;

use crate::sys::SYSTEM_TABLE;

#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    log::error!("memory allocation of {} bytes failed", layout.size());
    loop {}
}

struct UefiAlloc {}
#[global_allocator]
/// Global UEFI allocator. The allocations will have a memory type of `Bootinfo::UEFI_MEMORY_TYPE`.
/// This is useful as then the kernel can clean up all of these pages.
static ALLOCATOR: UefiAlloc = UefiAlloc {};

unsafe impl GlobalAlloc for UefiAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // We use the pool allocator in UEFI. Alignment is 8 bytes.
        if 8 % layout.align() != 0 {
            return ptr::null_mut();
        }

        match SYSTEM_TABLE
            .get()
            .boot_services()
            .allocate_pool(MemoryType::custom(UEFI_MEMORY_TYPE), layout.size())
        {
            Ok(ptr) => ptr.log(),
            Err(error) => {
                log::error!(
                    "Couldn't allocate pool for {:?}. Got error: {:?}",
                    layout,
                    error
                );
                ptr::null_mut()
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if let Err(e) = SYSTEM_TABLE.get().boot_services().free_pool(ptr) {
            log::error!(
                "Couldn't free pool at address {:p}. Got error: {:?}",
                ptr,
                e
            );
        }
    }
}
