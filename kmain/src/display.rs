use core::fmt;

use bootinfo::Framebuffer;
use framed::{Frame, Pixel};

#[derive(Debug)]
pub struct Display {
    framebuffer: Framebuffer,
}

impl Display {
    pub fn new(framebuffer: Framebuffer) -> Self {
        Self { framebuffer }
    }
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
