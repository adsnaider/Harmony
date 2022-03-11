//! Kernel entry and executable. Ideally, this is just a thin wrapper over all of the kernel's
//! components.
#![no_std]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]

mod display;
pub mod live_static;

use core::panic::PanicInfo;

use bootinfo::Bootinfo;
use display::Display;
use framed::console::{BitmapFont, Console};
use framed::{Frame, Pixel};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Can't do much about errors at this point.
    let _ = try_println!("{}", info);
    loop {}
}

/// Kernel's starting point.
#[no_mangle]
pub extern "C" fn kmain(bootinfo: &'static mut Bootinfo) -> ! {
    let mut display = Display::new(bootinfo.framebuffer);

    display.fill_with(Pixel::black());
    let font = match BitmapFont::decode_from(bootinfo.font) {
        Ok(font) => font,
        Err(_) => {
            display.fill_with(Pixel::red());
            loop {}
        }
    };
    display::init(Console::new(display, font));
    println!("Hello, Kernel!");
    log::info!("Hello, logging!");
    loop {}
}
