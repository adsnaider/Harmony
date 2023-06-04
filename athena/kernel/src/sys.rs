//! System management and functionality.

#[macro_use]
mod display;

use bootloader_api::info::{MemoryRegion, Optional};
use bootloader_api::BootInfo;
use critical_section::CriticalSection;
pub use display::_print;
use framed::console::{BitmapFont, Console};
use framed::{Frame, Pixel};

use self::display::Display;

const FONT: &[u8] = include_bytes!("../font.bdf");

/// System intialization routine.
///
/// It sets up the display, initializes the console, sets up the logger, memory utilities and
/// allocation, and it initializes interrupts.
///
/// # Safety
///
/// The information in `bootinfo` must be accurate.
pub(super) unsafe fn init(bootinfo: &mut BootInfo, cs: CriticalSection) {
    // SAFETY: Bootloader passed the framebuffer correctly.
    let framebuffer = core::mem::replace(&mut bootinfo.framebuffer, Optional::None)
        .into_option()
        .unwrap();
    let framebuffer_addr = framebuffer.buffer() as *const [u8];
    let mut display = unsafe { Display::new(framebuffer) };

    display.fill_with(Pixel::black());
    let font = match BitmapFont::decode_from(FONT) {
        Ok(font) => font,
        Err(_) => {
            display.fill_with(Pixel::red());
            panic!("Can't get display to work.");
        }
    };
    display::init(Console::new(display, font), cs);
    println!("Hello, Kernel!");
    log::info!("Hello, logging!");

    log::debug!(
        "Memory map starts at {:#?}",
        &*bootinfo.memory_regions as *const [MemoryRegion]
    );
    let pmo = bootinfo
        .physical_memory_offset
        .into_option()
        .expect("No memory offset found from bootloader.");
    log::debug!("Physical memory offset is {:#?}", pmo as *const ());
    log::debug!("Framebuffer mapped to {:#?}", framebuffer_addr);

    // SAFETY: The physical memory offset is correct, well-aligned, and canonical, and the memory
    // map is correct from the bootloader.
    unsafe { crate::arch::init(pmo, &mut bootinfo.memory_regions, cs) }
}
