//! UEFI allocation services.

use core::alloc::{GlobalAlloc, Layout};
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ptr::{self, NonNull};

use uefi::table::boot::{AllocateType, MemoryType};

use crate::sys::SYSTEM_TABLE;

#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    log::error!("memory allocation of {} bytes failed", layout.size());
    loop {}
}

/// UEFI page size in bytes.
pub const PAGE_SIZE: usize = 4096;

/// Attempts to get a specific memory range.
pub unsafe fn get_pages<'a>(
    address: Option<usize>,
    count: usize,
    memory_type: MemoryType,
) -> Result<&'a mut [u8], ()> {
    let pages = SYSTEM_TABLE
        .get()
        .boot_services()
        .allocate_pages(
            match address {
                Some(addr) => AllocateType::Address(addr),
                None => AllocateType::AnyPages,
            },
            memory_type,
            count,
        )
        .map_err(|_| ())?
        .log();

    #[cfg(debug_assertions)]
    if let Some(addr) = address {
        assert_eq!(pages, addr as u64);
    }
    Ok(core::slice::from_raw_parts_mut(
        pages as *mut u8,
        count * PAGE_SIZE,
    ))
}

#[derive(Debug)]
/// Arena allocator allows for data allocation onto a buffer. There's no deallocation. Instead, all
/// memory is freed with the lifetime of the arena.
pub struct Arena<'a> {
    /// We store buffer as raw pointer since we don't want mutable aliasing.
    buffer: *mut u8,
    /// Length of the buffer.
    size: usize,
    phantom: PhantomData<&'a [u8]>,
}

unsafe fn aligned_to_high(pointer: *mut u8, alignment: usize) -> *mut u8 {
    // (8 - 8 % 8) % 8 = 0;
    // (8 - 7 % 8) % 8 = 1;
    // (8 - 6 % 8) % 8 = 2;
    let offset = (alignment - pointer as usize % alignment) % alignment;
    pointer.add(offset)
}

/// Allocation Errors.
#[derive(Debug, Copy, Clone)]
pub enum AllocError {
    /// Returned when allocation fails due to out of memory.
    OutOfMemory,
}

impl<'a> Arena<'a> {
    /// Creates an Arena with the provided buffer. The arena doesn't control the lifetime of the
    /// buffer. Instead, the arena simply provides an abstraction over the buffer.
    pub fn new(buffer: &'a mut [u8]) -> Arena<'a> {
        Arena {
            buffer: buffer.as_mut_ptr(),
            size: buffer.len(),
            phantom: PhantomData,
        }
    }

    /// Allocates data into the arena given the `layout`.
    pub fn allocate(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if layout.size() > self.size {
            return Err(AllocError::OutOfMemory);
        }

        let aligned_buffer = unsafe { aligned_to_high(self.buffer, layout.align()) };
        let wasted_space = aligned_buffer as usize - self.buffer as usize;
        if layout.size() > self.size - wasted_space {
            return Err(AllocError::OutOfMemory);
        }
        self.size = self.size - wasted_space - layout.size();

        unsafe {
            self.buffer = aligned_buffer.add(layout.size());
            Ok(NonNull::slice_from_raw_parts(
                NonNull::new_unchecked(aligned_buffer),
                layout.size(),
            ))
        }
    }

    /// Allocates the value `value` into the arena and returns a mutable reference to the allocated
    /// memory if successful.
    pub fn allocate_value<T>(&mut self, value: T) -> Result<&'a mut T, AllocError> {
        let mut pointer: NonNull<T> = {
            let pointer = self.allocate(Layout::for_value(&value))?;
            assert!(pointer.len() >= core::mem::size_of::<T>());
            pointer.cast()
        };
        let handle: &mut MaybeUninit<T> = unsafe { pointer.as_uninit_mut() };
        Ok(handle.write(value))
    }

    /// Allocates a slice that can hold `length` elements oftype `T`. The return will be
    /// uninitialized.
    pub fn allocate_uninit_slice<T>(&mut self, length: usize) -> &'a mut [MaybeUninit<T>] {
        let pointer = self
            .allocate(Layout::array::<T>(length).unwrap())
            .expect("Out of memory.");
        unsafe { core::slice::from_raw_parts_mut(pointer.as_ptr() as *mut MaybeUninit<T>, length) }
    }
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
            .allocate_pool(MemoryType::LOADER_DATA, layout.size())
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
