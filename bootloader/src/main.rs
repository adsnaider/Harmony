#![no_std]
#![no_main]
#![feature(abi_efiapi)]

use core::panic::PanicInfo;

extern crate alloc as alloc_api;

use core::arch::asm;

use alloc_api::format;
use bootinfo::Bootinfo;
use bootloader::sys::alloc::Arena;
use bootloader::{sys, KERNEL_CODE_MEMORY, KERNEL_STACK_MEMORY, KERNEL_STATIC_MEMORY};
use goblin::elf;
use goblin::elf::program_header::PT_LOAD;
use goblin::elf32::program_header::pt_to_str;
use uefi::prelude::*;
use {bootinfo, log};

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

pub unsafe fn kernel_handoff(
    entry: usize,
    bootinfo: &'static mut Bootinfo,
    stack: &'static mut [u8],
) -> ! {
    asm!(
        "mov rsp, {}",
        "push 0", // Return pointer. (no return).
        "jmp {}",
        in(reg) stack.as_ptr_range().end as usize - 1,
        in(reg) entry,
        in("rdi") bootinfo as *mut Bootinfo as usize,
    );
    todo!();
}

#[entry]
fn efi_main(handle: Handle, system_table: SystemTable<Boot>) -> Status {
    // For the initial bootloader, it has to:
    // 1. Read the kernel program from disk.
    // 2. Get the framebuffer structure.
    // 3. Get the memory map.
    // 4. Load the kernel to memory.
    // 5. Run the kernel passing the boot data.
    sys::init(system_table);
    log::info!("Hello, UEFI!");
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
                    let page_start = aligned_to_low(header.vm_range().start, 4096);
                    let page_end = aligned_to_low(header.vm_range().end, 4096);
                    let count = page_end / 4096 - page_start / 4096 + 1;
                    log::info!("Requesting {} pages at {:#X}", count, page_start);
                    let memory = sys::alloc::get_pages(Some(page_start), count, KERNEL_CODE_MEMORY)
                        .expect(&format!(
                            "Couldn't get memory to load the kernel at address: {:#X} - {:#X}",
                            header.vm_range().start,
                            header.vm_range().end
                        ));
                    memory.iter_mut().for_each(|x| *x = 0);
                    // Memory range doesn't start at the beginning of page. Offset that.
                    let memory = &mut memory[(header.vm_range().start - page_start)..];
                    memory[..header.p_filesz as usize]
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
    log::info!("Jumping to kernel!");

    unsafe {
        let mut kernel_static: Arena<'static> = Arena::new(
            sys::alloc::get_pages(None, 8, KERNEL_STATIC_MEMORY)
                .expect("Couldn't allocate kernel static pages."),
        );
        let stack: &'static mut [u8] = sys::alloc::get_pages(None, 1024, KERNEL_STACK_MEMORY)
            .expect("Couldn't allocate kernel stack");

        let framebuffer = sys::io::get_framebuffer();
        // No more allocation services from here on.
        let (_runtime, memory_map) = sys::exit_uefi_services(handle, &mut kernel_static);
        let bootinfo = kernel_static
            .allocate_value(Bootinfo {
                framebuffer,
                memory_map,
            })
            .expect("Couldn't allocate bootinfo into statics.");
        kernel_handoff(entry as usize, bootinfo, stack);
    }
}
