//! UEFI allocation services.
//!
//! Once the global `SYSTEM_TABLE` is set, this module enables global allocation services, meaning
//! that operations using `Box` and `Vec` would work.

use core::alloc::{GlobalAlloc, Layout};
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ptr::{self, NonNull};

use uefi::table::boot::{AllocateType, MemoryType};

use crate::mem::aligned_to_high;
use crate::sys::SYSTEM_TABLE;

#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    log::error!("memory allocation of {} bytes failed", layout.size());
    #[allow(clippy::empty_loop)]
    loop {}
}

/// UEFI page size in bytes.
pub const PAGE_SIZE: usize = 4096;

/// Error returned when the bootloader wasn't able to allocate pages.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PageAllocError {}

/// Attempts to allocate a `count` pages and "labels" them with `memory_type`.
/// If `address` is given, then the returned buffer will start at `address`.
///
/// # Safety
///
/// The memory type used will be used by the kernel after boot. If the memory type doesn't match
/// the true memory type, then the kernel may deallocate pages that shouldn't be deallocated or
/// keep pages allocated that could be deallocated.
pub unsafe fn get_pages<'a>(
    address: Option<usize>,
    count: usize,
    memory_type: MemoryType,
) -> Result<&'a mut [u8], PageAllocError> {
    // SAFETY: The system is the only one with access to the system table.
    let pages = unsafe {
        SYSTEM_TABLE
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
            .map_err(|_| PageAllocError {})?
            .log()
    };

    #[cfg(debug_assertions)]
    if let Some(addr) = address {
        assert_eq!(pages, addr as u64);
    }
    // SAFETY: Got the memory allocated by the UEFI allocator.
    Ok(unsafe { core::slice::from_raw_parts_mut(pages as *mut u8, count * PAGE_SIZE) })
}

#[derive(Debug)]
/// Arena allocator allows for data allocation onto a buffer. There's no deallocation. Instead, all
/// memory is freed with the lifetime of the arena. In other words, the Arena doesn't manage the
/// memory. Instead, it's just a thin wrapper over the buffer.
pub struct Arena<'a> {
    /// We store buffer as raw pointer since we don't want mutable aliasing.
    buffer: *mut u8,
    /// Length of the buffer.
    size: usize,
    phantom: PhantomData<&'a [u8]>,
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

    /// Returns the amount of space in bytes left.
    pub fn remaining_size(&self) -> usize {
        self.size
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

    /// Allocates the `value` into the arena and returns a mutable reference to the allocated
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

    /// Allocates a slice that can hold `length` elements of type `T`. The return will be
    /// uninitialized.
    pub fn allocate_uninit_slice<T>(&mut self, length: usize) -> &'a mut [MaybeUninit<T>] {
        let pointer = self
            .allocate(Layout::array::<T>(length).unwrap())
            .expect("Out of memory.");
        unsafe { core::slice::from_raw_parts_mut(pointer.as_ptr() as *mut MaybeUninit<T>, length) }
    }

    /// Copies the content of the slice `in` into the arena and returns a mutable reference to it.
    pub fn allocate_and_copy_slice<T: Copy>(&mut self, from: &[T]) -> &'a mut [T] {
        let out = self.allocate_uninit_slice(from.len());
        MaybeUninit::write_slice(out, from)
    }
}

struct UefiAlloc {}

/// Global UEFI allocator. The allocations will have a memory type of `MemoryType::LOADER_DATA`.
#[global_allocator]
static ALLOCATOR: UefiAlloc = UefiAlloc {};

unsafe impl GlobalAlloc for UefiAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // We use the pool allocator in UEFI. Alignment is 8 bytes.
        if 8 % layout.align() != 0 {
            return ptr::null_mut();
        }

        // SAFETY: The system is the only one with access to the system table.
        match unsafe { SYSTEM_TABLE.get() }
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
        // SAFETY: The system is the only one with access to the system table.
        if let Err(e) = unsafe { SYSTEM_TABLE.get() }.boot_services().free_pool(ptr) {
            log::error!(
                "Couldn't free pool at address {:p}. Got error: {:?}",
                ptr,
                e
            );
        }
    }
}
