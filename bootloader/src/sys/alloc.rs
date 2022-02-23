//! UEFI allocation services.

use core::alloc::{GlobalAlloc, Layout};

#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    panic!("memory allocation of {} bytes failed", layout.size());
}

#[global_allocator]
static ALLOCATOR: UefiAlloc = UefiAlloc {};

struct UefiAlloc {}

unsafe impl GlobalAlloc for UefiAlloc {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        todo!();
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        todo!();
    }
}
