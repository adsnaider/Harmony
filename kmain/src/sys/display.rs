//! System display and console.

use bootinfo::Framebuffer;
use framed::console::Console;
use framed::{Frame, Pixel};
use log::{self, Level, LevelFilter, Metadata, Record};

use crate::singleton::Singleton;

/// The main console for the kernel.
static CONSOLE: Singleton<Console<Display>> = Singleton::uninit();
/// The global logger.
static LOGGER: DisplayLogger = DisplayLogger {};

/// Initializes the console and logger. It's reasonable to use the print!, try_print!,
/// println!, try_println!, and log::* macros after this call.
pub(super) fn init(console: Console<Display>) {
    CONSOLE.initialize(console);
    if let Err(e) = log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Info)) {
        crate::println!("Couldn't initialize logging services: {e}");
    }
}

/// The display struct implements the `Frame` trait from the framebuffer pointer.
#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub struct Display {
    framebuffer: Framebuffer,
}

// SAFETY: Precondition for creating the display prevents multiple frame buffers from existing in
// the system.
unsafe impl Send for Display {}

impl Display {
    /// Create a new display with the framebuffer.
    ///
    /// # Safety
    ///
    /// * The framebuffer must be correct.
    /// * There should only be one framebuffer (i.e. the memory in the framebuffer is now owned
    /// by the display).
    pub unsafe fn new(framebuffer: Framebuffer) -> Self {
        Self { framebuffer }
    }
}

// SAFETY: We correctly define the width and height of the display since the framebuffer is correct
// (precondition).
unsafe impl Frame for Display {
    unsafe fn set_pixel_unchecked(&mut self, row: usize, col: usize, pixel: Pixel) {
        match self.framebuffer.pixel_format {
            bootinfo::PixelFormat::Rgb => {
                // Each pixel has 4 bytes.
                const PIXEL_SIZE: usize = 4;
                let offset = row * self.framebuffer.stride * PIXEL_SIZE + col * PIXEL_SIZE;
                let color: u32 =
                    ((pixel.blue as u32) << 16) + ((pixel.green as u32) << 8) + (pixel.red as u32);
                // SAFETY: The framebuffer structure is correct (precondition).
                unsafe {
                    core::ptr::write_volatile(
                        self.framebuffer.address.add(offset) as *mut u32,
                        color,
                    )
                };
            }
            bootinfo::PixelFormat::Bgr => {
                // Each pixel has 4 bytes.
                const PIXEL_SIZE: usize = 4;
                let offset = row * self.framebuffer.stride * PIXEL_SIZE + col * PIXEL_SIZE;
                let color: u32 =
                    ((pixel.red as u32) << 16) + ((pixel.green as u32) << 8) + (pixel.blue as u32);
                // SAFETY: The framebuffer structure is correct (precondition).
                unsafe {
                    core::ptr::write_volatile(
                        self.framebuffer.address.add(offset) as *mut u32,
                        color,
                    )
                };
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
    ($($arg:tt)*) => {$crate::sys::_print(format_args!($($arg)*))};
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
    ($($arg:tt)*) => {$crate::sys::_try_print(format_args!($($arg)*))};
}

/// Prints the arguments to the console and moves to the next line. Returns an error on failure.
#[macro_export]
macro_rules! try_println {
    () => ($crate::try_print!("\n"));
    ($($arg:tt)*) => ($crate::try_print!("{}\n", format_args!($($arg)*)));
}

/// Prints the arguments to the screen, panicking if unable to.
#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    CONSOLE.lock().write_fmt(args).unwrap();
}

/// Returned when the try_print! macros fail to print to the screen.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PrintError {
    /// Couldn't print due to a borrowing error (in use)
    LockError,
    /// General fmt error.
    PrintError,
    /// Console hasn't been initialized.
    UninitError,
}

/// Tries to print the args using the default printer.
///
/// # Errors
///
/// * If the console is currently in use.
/// * If the console wasn't able to print the arguments.
#[doc(hidden)]
pub fn _try_print(args: core::fmt::Arguments) -> Result<(), PrintError> {
    use core::fmt::Write;
    CONSOLE
        .try_lock()
        .ok_or(PrintError::LockError)?
        .write_fmt(args)
        .map_err(|_| PrintError::PrintError)
}
