//! Kernel entry and executable. Ideally, this is just a thin wrapper over all of the kernel's
//! components.
#![no_std]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(missing_copy_implementations)]

use core::cell::RefCell;
use core::fmt::Write;
use core::panic::PanicInfo;

use bootinfo::Bootinfo;
use framed::console::{BitmapFont, Console};
use framed::{Frame, Pixel};

mod display;
pub(crate) mod live_static;

use display::Display;
use live_static::LiveStatic;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Ok(mut console) = CONSOLE.try_borrow_mut() {
        writeln!(console, "{}", info);
    }
    loop {}
}

static CONSOLE: LiveStatic<Console<Display>> = LiveStatic::new();

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
    CONSOLE.set(Console::new(display, font));
    writeln!(CONSOLE.borrow_mut(), "Hello, Kernel!");

    loop {}
}
