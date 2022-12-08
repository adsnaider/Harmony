//! System management and functionality.

use alloc::boxed::Box;

use bootinfo::{Bootinfo, MemoryRegion};
use framed::console::{BitmapFont, Console};
use framed::{Frame, Pixel};
use x86_64::VirtAddr;

#[macro_use]
mod display;
mod drivers;
mod gdt;
mod interrupts;
mod memory;

pub mod time;

pub use display::{_print, _try_print};

use self::display::Display;

/// System intialization routine.
///
/// It sets up the display, initializes the console, sets up the logger, memory utilities and
/// allocation, and it initializes interrupts.
///
/// # Safety
///
/// The information in `bootinfo` must be accurate.
pub(super) unsafe fn init(bootinfo: &'static mut Bootinfo) {
    critical_section::with(|_token| {
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

        log::debug!("Found bootinfo at {:#?}", bootinfo as *const Bootinfo);
        log::debug!("Found framebuffer at {:#?}", bootinfo.framebuffer.address);
        log::debug!("Found font at {:#?}", bootinfo.font as *const [u8]);
        log::debug!(
            "Memory map starts at {:#?}",
            bootinfo.memory_map.regions as *const [MemoryRegion]
        );
        log::debug!(
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

        time::init(drivers::take_pit().unwrap());

        gdt::init();
        interrupts::init();
        log::info!("Interrupt handlers initialized");
    });
    x86_64::instructions::interrupts::enable();
}

struct SingleThreadCS();
critical_section::set_impl!(SingleThreadCS);
/// SAFETY: While the OS kernel is running in a single thread, then disabling interrupts is a safe
/// to guarantee a critical section's conditions.
unsafe impl critical_section::Impl for SingleThreadCS {
    unsafe fn acquire() -> critical_section::RawRestoreState {
        let interrupts_enabled = x86_64::instructions::interrupts::are_enabled();
        x86_64::instructions::interrupts::disable();
        interrupts_enabled
    }

    unsafe fn release(interrupts_were_enabled: critical_section::RawRestoreState) {
        if interrupts_were_enabled {
            x86_64::instructions::interrupts::enable();
        }
    }
}
