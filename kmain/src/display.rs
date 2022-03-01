use bootinfo::Framebuffer;
use framed::console::Console;
use framed::{Frame, Pixel};
use log::{self, Level, LevelFilter, Metadata, Record};

use crate::live_static::{LiveStatic, StaticBorrowError};

/// The main console for the kernel.
static CONSOLE: LiveStatic<Console<Display>> = LiveStatic::new();

/// The global logger.
static LOGGER: DisplayLogger = DisplayLogger {};

/// Initializes the console and logger. It's reasonable to use the print!, try_print!,
/// println!, try_println!, and log::* macros after this call.
pub(super) fn init(console: Console<Display>) {
    CONSOLE.set(console);
    if let Err(e) = log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Info)) {
        crate::println!("Couldn't initialize logging services: {e}");
    }
}

/// The display struct implements the `Frame` trait from the framebuffer pointer.
#[derive(Debug)]
pub struct Display {
    framebuffer: Framebuffer,
}

impl Display {
    /// Create a new display with the framebuffer.
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

struct DisplayLogger;

impl log::Log for DisplayLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            crate::println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

/// Prints the arguments to the console. May panic!.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {$crate::display::_print(format_args!($($arg)*))};
}

/// Prints the arguments to the console and moves to the next line. May panic!.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

/// Prints the arguments to the console. Returns an error on failure.
#[macro_export]
macro_rules! try_print {
    ($($arg:tt)*) => {$crate::display::_try_print(format_args!($($arg)*))};
}

/// Prints the arguments to the console and moves to the next line. Returns an error on failure.
#[macro_export]
macro_rules! try_println {
    () => ($crate::try_print!("\n"));
    ($($arg:tt)*) => ($crate::try_print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    CONSOLE.borrow_mut().write_fmt(args).unwrap();
}

pub enum PrintError {
    BorrowError(StaticBorrowError),
    PrintError,
}

#[doc(hidden)]
pub fn _try_print(args: core::fmt::Arguments) -> Result<(), PrintError> {
    use core::fmt::Write;
    CONSOLE
        .try_borrow_mut()
        .map_err(|e| PrintError::BorrowError(e))?
        .write_fmt(args)
        .map_err(|_| PrintError::PrintError)
}