#![no_std]
#![no_main]
#![feature(abi_efiapi)]

use core::panic::PanicInfo;

extern crate alloc as alloc_api;

use core::arch::asm;

use bootinfo::Bootinfo;
use bootloader::arch::x86_64_utils::PageFrameAllocator;
use bootloader::sys::alloc::Arena;
use bootloader::{sys, KERNEL_CODE_MEMORY, KERNEL_STACK_MEMORY, KERNEL_STATIC_MEMORY};
use goblin::elf;
use goblin::elf::program_header::PT_LOAD;
use goblin::elf32::program_header::pt_to_str;
use uefi::prelude::*;
use uefi::table::boot::MemoryType;
use x86_64::structures::paging::{Mapper, Page, PageSize, PageTableFlags, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

/// Enough for 16TB physical memory section.
const PHYSICAL_MEMORY_OFFSET: usize = 0xFFFF_F000_0000_0000;
/// Number of pages used for statics passed by bootloader.
const KERNEL_STATICS_PAGE_COUNT: usize = 16;
/// Bottom page used for kernel statics.
const KERNEL_STATICS_BOTTOM: usize = PHYSICAL_MEMORY_OFFSET - KERNEL_STATICS_PAGE_COUNT * 4096;
/// Top of the stack lives right below the passed statics.
const STACK_TOP: usize = KERNEL_STATICS_BOTTOM;
/// Number of pages to give the kernel stack.
const STACK_PAGES: usize = 1024;
/// Bottom of the kernel stack.
const STACK_BOTTOM: usize = STACK_TOP - STACK_PAGES * 4096;

// TODO(#6): Move to utils crate.
fn aligned_to_low(address: usize, alignment: usize) -> usize {
    let offset = address % alignment;
    address - offset
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if sys::is_init() {
        log::error!("{}", info);
    }
    loop {}
}

/// Jumps into the kernel's entry point.
///
/// # Safety
///
/// * The entry address must be correct and match the in-memory kernel. The kernel also needs to be
/// correctly mapped given the ELF specification.
/// * The entry function should be take have the signature `fn(&'static mut Bootinfo) -> !`.
pub unsafe fn kernel_handoff(
    entry: usize,
    bootinfo: &'static mut Bootinfo,
    stack: &'static mut [u8],
) -> ! {
    const _: () = {
        assert!(STACK_TOP % 16 == 0);
    };
    asm!(
        "mov rsp, {}",
        "push 0", // Return pointer. (no return).
        "jmp {}",
        in(reg) stack.as_ptr_range().end as usize,
        in(reg) entry,
        in("rdi") bootinfo as *mut Bootinfo as usize,
    );
    unreachable!();
}

struct MappedAllocation<'a> {
    identity_buffer: &'a mut [u8],
    mapped_buffer: &'a mut [u8],
}

/// Remaps a chunk of contiguous pages to a chunk of contiguous frames.
///
/// # Safety
///
/// Mapping pages is fundamentally unsafe.
unsafe fn allocate_mapped<S: PageSize>(
    virtual_address_start: Page<S>,
    count: usize,
    memory_type: MemoryType,
    page_table_flags: PageTableFlags,
    page_map: &mut impl Mapper<S>,
) -> MappedAllocation<'static> {
    let mut page_frame_allocator = PageFrameAllocator {};

    let buffer = sys::alloc::get_pages(None, count, memory_type).expect("Allocation failure");
    let physical_address_start = unsafe { PhysAddr::new_unsafe(buffer.as_ptr() as u64) };
    let virtual_address_start = virtual_address_start.start_address();

    for i in 0..count {
        // SAFETY: address will still be aligned since we are adding multiple of S::SIZE.
        let page: Page<S> = unsafe {
            Page::from_start_address_unchecked(virtual_address_start + (i as u64 * S::SIZE))
        };
        // SAFETY: address will still be aligned since we are adding multiple of S::SIZE.
        let frame = unsafe {
            PhysFrame::from_start_address_unchecked(physical_address_start + (i as u64 * S::SIZE))
        };
        // SAFETY: Function precondition.
        unsafe {
            page_map
                .map_to(page, frame, page_table_flags, &mut page_frame_allocator)
                .unwrap_or_else(|_| panic!("Couldn't map {page:?} to {frame:?}"))
                .flush();
        }
    }
    // SAFETY: Allocated and mapped to virtual address.
    let mapped_buffer = unsafe {
        core::slice::from_raw_parts_mut(
            virtual_address_start.as_mut_ptr(),
            count * S::SIZE as usize,
        )
    };
    MappedAllocation {
        identity_buffer: buffer,
        mapped_buffer,
    }
}

#[entry]
fn efi_main(handle: Handle, system_table: SystemTable<Boot>) -> Status {
    // For the initial bootloader, it has to:
    // 1. Read the kernel program from disk.
    // 2. Get the framebuffer structure.
    // 3. Get the font.
    // 4. Get the memory map.
    // 5. Load the kernel to memory.
    // 6. Run the kernel passing the boot data.
    sys::init(system_table);
    log::info!("Hello, UEFI!");

    // SAFETY: We are mapping all memory well beyond the physical addresses used in identity mapping.
    let mut page_map = unsafe {
        // Remap all memory to `PHYSICAL_MEMORY_OFFSET` while also keeping identity mapping.
        bootloader::arch::x86_64_utils::remap_memory_to_offset(PHYSICAL_MEMORY_OFFSET)
    };

    let entry = {
        let kernel = sys::fs::read("kernel").expect("Can't read kernel file.");
        log::info!("Got kernel.");
        let elf =
            elf::Elf::parse(&kernel).expect("Couldn't parse the kernel as an ELF executable.");

        assert!(elf.is_64);
        assert!(elf.entry > 0);
        for header in elf.program_headers {
            match header.p_type {
                PT_LOAD => unsafe {
                    let mut flags = PageTableFlags::PRESENT;
                    if !header.is_executable() {
                        flags |= PageTableFlags::NO_EXECUTE;
                    }
                    if header.is_write() {
                        flags |= PageTableFlags::WRITABLE;
                    }

                    let page_start = aligned_to_low(header.vm_range().start, 4096);
                    let page_end = aligned_to_low(header.vm_range().end - 1, 4096);
                    let count = page_end / 4096 - page_start / 4096 + 1;
                    log::info!(
                        "Requesting {} pages to be loaded at {:#X}",
                        count,
                        page_start
                    );
                    // We don't need the virtual pointer for this and using the identity buffer
                    // us to directly set up the appropriate page table flags.
                    let segment = allocate_mapped::<Size4KiB>(
                        Page::from_start_address(VirtAddr::new(page_start as u64)).unwrap(),
                        count,
                        KERNEL_CODE_MEMORY,
                        flags,
                        &mut page_map,
                    )
                    .identity_buffer;

                    segment.iter_mut().for_each(|x| *x = 0);
                    // Memory range doesn't start at the beginning of page. Offset that.
                    let segment = &mut segment[(header.vm_range().start - page_start)..];
                    segment[..header.p_filesz as usize]
                        .copy_from_slice(&kernel[header.file_range()]);
                },
                other => log::info!(
                    "Found non-loadable program header of type: {}",
                    pt_to_str(other)
                ),
            }
        }
        elf.entry
    };
    log::info!("Kernel loaded!");

    let font = sys::fs::read("font.bdf")
        .expect("Couldn't read font file.")
        .leak();
    assert_eq!(font.len(), 256 * 16);
    log::info!("Loaded font file.");

    log::info!("Jumping to kernel!");

    unsafe {
        let mut kernel_static: Arena<'static> = Arena::new(
            allocate_mapped::<Size4KiB>(
                Page::from_start_address(VirtAddr::new(KERNEL_STATICS_BOTTOM as u64)).unwrap(),
                KERNEL_STATICS_PAGE_COUNT,
                KERNEL_STATIC_MEMORY,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
                &mut page_map,
            )
            .mapped_buffer,
        );

        let stack: &'static mut [u8] = allocate_mapped::<Size4KiB>(
            Page::from_start_address(VirtAddr::new(STACK_BOTTOM as u64)).unwrap(),
            STACK_PAGES,
            KERNEL_STACK_MEMORY,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
            &mut page_map,
        )
        .mapped_buffer;

        // TODO: Remap frame buffer and other MMIO?
        let framebuffer = sys::io::get_framebuffer();

        let font = kernel_static.allocate_and_copy_slice(font);

        log::info!("Ready to jump!");

        // No more allocation services from here on.
        let (_runtime, memory_map) = sys::exit_uefi_services(handle, &mut kernel_static);

        let bootinfo = kernel_static
            .allocate_value(Bootinfo {
                framebuffer,
                memory_map,
                physical_memory_offset: PHYSICAL_MEMORY_OFFSET,
                font,
            })
            .expect("Couldn't allocate bootinfo into statics.");
        kernel_handoff(entry as usize, bootinfo, stack);
    }
}
