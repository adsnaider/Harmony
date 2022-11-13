//! Kernel entry and executable. Ideally, this is just a thin wrapper over all of the kernel's
//! components.
#![no_std]
#![feature(abi_x86_interrupt)]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks)]

pub mod display;
pub mod interrupts;
pub(crate) mod singleton;

use bootinfo::Bootinfo;
use display::Display;
use framed::console::{BitmapFont, Console};
use framed::{Frame, Pixel};

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // Can't do much about errors at this point.
    let _ = try_println!("{}", info);
    loop {}
}

fn system_init(bootinfo: &'static mut Bootinfo) {
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

    interrupts::init();
    log::info!("Interrupt handlers initialized");
}

/// Kernel's starting point.
#[no_mangle]
pub extern "C" fn kmain(bootinfo: &'static mut Bootinfo) -> ! {
    system_init(bootinfo);

    log::info!("Initialization sequence complete.");

    #[allow(clippy::empty_loop)]
    loop {}
}
