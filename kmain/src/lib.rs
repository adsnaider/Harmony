//! Kernel entry and executable. Ideally, this is just a thin wrapper over all of the kernel's
//! components.
#![no_std]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(missing_copy_implementations)]

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

#[no_mangle]
/// Kernel's starting point.
pub extern "C" fn kmain(bootinfo: &'static mut Bootinfo) -> ! {
    let mut display = Display::new(bootinfo.framebuffer);

    display.fill_with(Pixel {
        red: 0,
        green: 0,
        blue: 0,
    });
    let font = match BitmapFont::decode_from(bootinfo.font) {
        Ok(font) => font,
        Err(_) => {
            display.fill_with(Pixel {
                red: 255,
                green: 0,
                blue: 0,
            });
            loop {}
        }
    };
    display::init(Console::new(display, font));
    println!("Hello, Kernel!");
    loop {}
}
