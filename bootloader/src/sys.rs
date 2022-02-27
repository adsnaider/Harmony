//! UEFI System operations and resources.
//!
//! This module defines ways to interact with the system while in the UEFI environment. In
//! particular, this module provides the `init()` operation that, takes possesion of the system
//! table. After the call to `init()`, the system will enable the global allocator and logging
//! services, such that a program can use Rust's alloc API including heap-managed containers such
//! as `Vec` and `Box`. Logging can be done using the `log` crate.
//!
//! Before jumping to the kernel, the system should exit the UEFI environment by calling
//! `exit_boot_services()`. This operation will exit the UEFI environment, meaning that allocation
//! and logging won't be available anymore afterwards. The operation returns the UEFI runtime table
//! that can be used after exiting boot services alongside the current memory map. Both of these
//! can be passed to the kernel.
pub mod alloc;
pub mod fs;
pub mod io;

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

use bootinfo::{MemoryMap, MemoryRegion};
use uefi::data_types::Align;
use uefi::table::boot::{MemoryDescriptor, MemoryMapSize, MemoryType};
use uefi::table::{Boot, Runtime, SystemTable};
use uefi::{Handle, ResultExt};

use self::alloc::Arena;
use crate::mem::aligned_to_high;
use crate::{KERNEL_CODE_MEMORY, KERNEL_STACK_MEMORY, KERNEL_STATIC_MEMORY};

/// GlobalTable holds a reference to the UEFI system table.
pub(crate) struct GlobalTable {
    /// Reference to the system table.
    table: UnsafeCell<Option<SystemTable<Boot>>>,
}
// SAFETY: Not safe, but UEFI has no multi-threading support.
unsafe impl Sync for GlobalTable {}

/// System table used by the rest of the system. In order for get/get_mut to be safe, each part of
/// the code should only access the specific sub-system that they have access to.
///
/// For instance, the logging system, can access stdout(), and the framebuffer can access gop().
// TODO(asnaider): Is the safety explanation sound? Can both subsystems be accessed at the same
// time?
pub(crate) static SYSTEM_TABLE: GlobalTable = GlobalTable {
    table: UnsafeCell::new(None),
};

impl GlobalTable {
    /// Get a reference to the system table if setup. Otherwise, panic.
    ///
    /// # Safety
    /// Aliasing rules apply.
    pub unsafe fn get(&self) -> &SystemTable<Boot> {
        (&*self.table.get())
            .as_ref()
            .expect("System table hasn't been initialized. Forget to call `init()`?")
    }

    /// Get a mutable reference to the system table if setup. Otherwise, panic.
    ///
    /// # Safety
    /// Aliasing rules apply.
    pub unsafe fn get_mut(&self) -> &mut SystemTable<Boot> {
        (&mut *self.table.get())
            .as_mut()
            .expect("System table hasn't been initialized. Forget to call `init()`?")
    }

    /// Sets the system table from the appropriate value.
    ///
    /// # Safety:
    /// Aliasing rules apply. In particular, there shouldn't be living references to the previous
    /// system table.
    unsafe fn set(&self, table: SystemTable<Boot>) {
        *self.table.get() = Some(table)
    }

    /// Returns whether there's a table set.
    ///
    /// # Safety:
    /// This will borrow the table immutably. There can't be mutable references to the system
    /// table.
    unsafe fn is_set(&self) -> bool {
        (&*self.table.get()).is_some()
    }
}

/// Initializes the UEFI system. After this call, it's possible to use allocation services and
/// logging.
pub fn init(system_table: SystemTable<Boot>) {
    unsafe {
        if SYSTEM_TABLE.is_set() {
            panic!("Attempt to call sys::init() twice.");
        }
        SYSTEM_TABLE.set(system_table);
    }

    io::init();
}

/// Returns true if the UEFI system has been initialized with a call to `init()`.
pub fn is_init() -> bool {
    unsafe { SYSTEM_TABLE.is_set() }
}

/// After this call, UEFI system services will become unavailable. The function also returns UEFI
/// runtime table and the current memory map.
pub fn exit_uefi_services(
    handle: Handle,
    statics: &mut Arena<'static>,
) -> (SystemTable<Runtime>, MemoryMap<'static>) {
    // This will disable all of the system functions.
    let table = unsafe {
        (*SYSTEM_TABLE.table.get())
            .take()
            .expect("No system table found. Forget to call `init()`?")
    };
    let memory_map_buf = {
        // Extra buffer since the size might change.
        let MemoryMapSize {
            map_size: total,
            entry_size: entry,
        } = table.boot_services().memory_map_size();
        let size = total + entry * 3;
        // TODO(asnaider): Maybe deallocate pool. Not a huge deal as the kernel can discard
        // LOADER_DATA memory anyway.
        let address = table
            .boot_services()
            .allocate_pool(MemoryType::LOADER_DATA, size)
            .expect_success("Couldn't allocate data for memory map.");
        let address = unsafe { aligned_to_high(address, MemoryDescriptor::alignment()) };

        unsafe {
            let buf = core::slice::from_raw_parts_mut(address, size);
            MemoryDescriptor::assert_aligned(buf);
            buf
        }
    };
    let (runtime, descriptors) = table
        .exit_boot_services(handle, memory_map_buf)
        .expect_success("Couldn't exit boot services.");

    let regions: &'static mut [MaybeUninit<MemoryRegion>] =
        statics.allocate_uninit_slice(descriptors.len());

    for (i, desc) in descriptors.enumerate() {
        regions[i].write(MemoryRegion {
            ty: match desc.ty {
                MemoryType::RESERVED => bootinfo::MemoryType::Reserved,
                MemoryType::LOADER_DATA => bootinfo::MemoryType::UefiAvailable,
                MemoryType::LOADER_CODE => bootinfo::MemoryType::UefiAvailable,
                MemoryType::BOOT_SERVICES_CODE => bootinfo::MemoryType::UefiAvailable,
                MemoryType::BOOT_SERVICES_DATA => bootinfo::MemoryType::UefiAvailable,
                MemoryType::RUNTIME_SERVICES_CODE => bootinfo::MemoryType::UefiUnavailable,
                MemoryType::RUNTIME_SERVICES_DATA => bootinfo::MemoryType::UefiUnavailable,
                MemoryType::CONVENTIONAL => bootinfo::MemoryType::Conventional,
                MemoryType::UNUSABLE => bootinfo::MemoryType::Reserved,
                MemoryType::ACPI_RECLAIM => bootinfo::MemoryType::AcpiReclaim,
                MemoryType::ACPI_NON_VOLATILE => bootinfo::MemoryType::AcpiUnavailable,
                MemoryType::MMIO => bootinfo::MemoryType::Mmio,
                MemoryType::MMIO_PORT_SPACE => bootinfo::MemoryType::MmioPort,
                MemoryType::PAL_CODE => bootinfo::MemoryType::Reserved,
                MemoryType::PERSISTENT_MEMORY => bootinfo::MemoryType::Persistent,
                KERNEL_STACK_MEMORY => bootinfo::MemoryType::KernelStack,
                KERNEL_STATIC_MEMORY => bootinfo::MemoryType::KernelData,
                KERNEL_CODE_MEMORY => bootinfo::MemoryType::KernelCode,
                other => panic!("Unknown memory type: {:?}", other),
            },
            phys_start: desc.phys_start as usize,
            page_count: desc.page_count as usize,
            attribute: match desc.att {
                _ => bootinfo::MemoryAttribute::Unknown,
            },
        });
    }
    unsafe {
        (
            runtime,
            MemoryMap {
                regions: MaybeUninit::slice_assume_init_mut(regions),
            },
        )
    }
}
