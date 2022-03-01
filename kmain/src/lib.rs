//! Kernel entry and executable. Ideally, this is just a thin wrapper over all of the kernel's
//! components.
#![no_std]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(missing_copy_implementations)]

use core::fmt::Write;
use core::panic::PanicInfo;

use bootinfo::{Bootinfo, Framebuffer};
use framed::console::{BitmapFont, Console};
use framed::{Frame, Pixel};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[derive(Debug)]
struct Display {
    framebuffer: Framebuffer,
}

unsafe impl Frame for Display {
    unsafe fn set_pixel_unchecked(&mut self, row: usize, col: usize, pixel: Pixel) {
        match self.framebuffer.pixel_format {
            bootinfo::PixelFormat::Rgb => {
                // Each pixel has 4 bytes.
                const PIXEL_SIZE: usize = 4;
                let offset = row * self.framebuffer.stride * PIXEL_SIZE + col * PIXEL_SIZE;
                let color: u32 =
                    ((pixel.blue as u32) << 16) + ((pixel.green as u32) << 8) + (pixel.red as u32);
                core::ptr::write_volatile(self.framebuffer.address.add(offset) as *mut u32, color);
            }
            bootinfo::PixelFormat::Bgr => {
                // Each pixel has 4 bytes.
                const PIXEL_SIZE: usize = 4;
                let offset = row * self.framebuffer.stride * PIXEL_SIZE + col * PIXEL_SIZE;
                let color: u32 =
                    ((pixel.red as u32) << 16) + ((pixel.green as u32) << 8) + (pixel.blue as u32);
                core::ptr::write_volatile(self.framebuffer.address.add(offset) as *mut u32, color);
            }
            _ => todo!(),
        }
    }

    fn width(&self) -> usize {
        self.framebuffer.resolution.0
    }

    fn height(&self) -> usize {
        self.framebuffer.resolution.1
    }
}

#[no_mangle]
/// Kernel's starting point.
pub extern "C" fn kmain(bootinfo: &'static mut Bootinfo) -> ! {
    let mut display = Display {
        framebuffer: bootinfo.framebuffer,
    };

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
    let mut console = Console::new(display, font);
    let _ = writeln!(console, "Hello, Kernel!");
    let _ = writeln!(console, "Hello, Kernel!");
    let _ = writeln!(console, "Hello, Kernel!");
    let _ = writeln!(console, "Hello, Kernel!");
    let _ = writeln!(console, "Hello, Kernel!");
    let _ = writeln!(console, "Hello, Kernel!");

    loop {}
}
