//! Kernel entry and executable. Ideally, this is just a thin wrapper over all of the kernel's
//! components.
#![no_std]
#![feature(allocator_api)]
#![feature(default_alloc_error_handler)]
#![feature(abi_x86_interrupt)]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks)]

pub mod display;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub(crate) mod singleton;

#[macro_use]
extern crate alloc;

use alloc::boxed::Box;

use bootinfo::{Bootinfo, MemoryRegion};
use display::Display;
use framed::console::{BitmapFont, Console};
use framed::{Frame, Pixel};
use x86_64::VirtAddr;

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // Can't do much about errors at this point.
    let _ = try_println!("{}", info);
    loop {}
}

fn system_init(bootinfo: &'static mut Bootinfo) {
    x86_64::instructions::interrupts::disable();
    // SAFETY: Bootloader passed the framebuffer correctly.
    let mut display = unsafe { Display::new(bootinfo.framebuffer) };

    display.fill_with(Pixel::black());
    let font = match BitmapFont::decode_from(bootinfo.font) {
        Ok(font) => font,
        Err(_) => {
            display.fill_with(Pixel::red());
            panic!("Can't get display to work.");
        }
    };
    display::init(Console::new(display, font));
    println!("Hello, Kernel!");
    log::info!("Hello, logging!");

    log::info!("Found bootinfo at {:#?}", bootinfo as *const Bootinfo);
    log::info!("Found framebuffer at {:#?}", bootinfo.framebuffer.address);
    log::info!("Found font at {:#?}", bootinfo.font as *const [u8]);
    log::info!(
        "Memory map starts at {:#?}",
        bootinfo.memory_map.regions as *const [MemoryRegion]
    );
    log::info!(
        "Physical memory offset is {:#?}",
        bootinfo.physical_memory_offset as *mut ()
    );

    // SAFETY: The physical memory offset is correct, well-aligned, and canonical, and the memory
    // map is correct from the bootloader.
    unsafe {
        memory::init(
            VirtAddr::new_unsafe(bootinfo.physical_memory_offset as u64),
            core::mem::take(&mut bootinfo.memory_map),
        );
    }

    let boxed_value = Box::new(25);
    log::info!("We are boxing! {boxed_value:?}");
    let vec_value = vec![1, 2, 3, 4, 5, 6];
    log::info!("And we are vecing: {vec_value:?}");
    let huge_vec = vec![0u64; 100000];
    for x in huge_vec.iter() {
        // SAFETY: Pointer is valid as we construct it from reference.
        unsafe { core::ptr::read_volatile(x as *const u64) };
    }
    log::info!("Allocated a huge vector!");

    gdt::init();
    interrupts::init();
    log::info!("Interrupt handlers initialized");
    x86_64::instructions::interrupts::enable();
}

/// Kernel's starting point.
#[no_mangle]
pub extern "C" fn kmain(bootinfo: &'static mut Bootinfo) -> ! {
    system_init(bootinfo);
    log::info!("Initialization sequence complete.");

    loop {
        x86_64::instructions::hlt();
    }
}
