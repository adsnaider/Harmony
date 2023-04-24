//! System display and console.

use atomic_refcell::{AtomicRefCell, BorrowMutError};
use bootloader_api::info::{FrameBuffer, PixelFormat};
use framed::console::Console;
use framed::{Frame, Pixel};
use log::{self, LevelFilter, Metadata, Record};
use once_cell::unsync::Lazy;

/// The main console for the kernel.
static CONSOLE: AtomicRefCell<Option<Console<Display>>> = AtomicRefCell::new(None);
/// The global logger.
static LOGGER: DisplayLogger = DisplayLogger {};

const LOG_LEVEL: Lazy<LevelFilter> = Lazy::new(|| {
    let level = option_env!("KERNEL_LOG_LEVEL").unwrap_or("info");
    match level {
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        other => panic!("Unknown LOG LEVEL: {other}"),
    }
});

/// Initializes the console and logger. It's reasonable to use the print!,
/// println! and log::* macros after this call.
pub(super) fn init(console: Console<Display>) {
    *CONSOLE.borrow_mut() = Some(console);
    if let Err(e) = log::set_logger(&LOGGER).map(|()| log::set_max_level(*LOG_LEVEL)) {
        crate::println!("Couldn't initialize logging services: {e}");
    }
}

/// The display struct implements the `Frame` trait from the framebuffer pointer.
#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub struct Display {
    framebuffer: FrameBuffer,
}

// SAFETY: Precondition for creating the display prevents multiple frame buffers from existing in
// the system.
unsafe impl Send for Display {}
/// SAFETY: Only 1 framebuffer exists and we require a mutable reference to write to the display.
unsafe impl Sync for Display {}

impl Display {
    /// Create a new display with the framebuffer.
    ///
    /// # Safety
    ///
    /// * The framebuffer must be correct.
    /// * There should only be one framebuffer (i.e. the memory in the framebuffer is now owned
    /// by the display).
    pub unsafe fn new(framebuffer: FrameBuffer) -> Self {
        Self { framebuffer }
    }
}

// SAFETY: We correctly define the width and height of the display since the framebuffer is correct
// (precondition).
unsafe impl Frame for Display {
    unsafe fn set_pixel_unchecked(&mut self, row: usize, col: usize, pixel: Pixel) {
        match self.framebuffer.info().pixel_format {
            PixelFormat::Rgb => {
                // Each pixel has 4 bytes.
                const PIXEL_SIZE: usize = 4;
                let offset = row * self.framebuffer.info().stride * PIXEL_SIZE + col * PIXEL_SIZE;
                let color: u32 =
                    ((pixel.blue as u32) << 16) + ((pixel.green as u32) << 8) + (pixel.red as u32);
                // SAFETY: The framebuffer structure is correct (precondition).
                unsafe {
                    core::ptr::write_volatile(
                        self.framebuffer.buffer_mut().as_mut_ptr().add(offset) as *mut u32,
                        color,
                    )
                };
            }
            PixelFormat::Bgr => {
                // Each pixel has 4 bytes.
                const PIXEL_SIZE: usize = 4;
                let offset = row * self.framebuffer.info().stride * PIXEL_SIZE + col * PIXEL_SIZE;
                let color: u32 =
                    ((pixel.red as u32) << 16) + ((pixel.green as u32) << 8) + (pixel.blue as u32);
                // SAFETY: The framebuffer structure is correct (precondition).
                unsafe {
                    core::ptr::write_volatile(
                        self.framebuffer.buffer_mut().as_mut_ptr().add(offset) as *mut u32,
                        color,
                    )
                };
            }
            _ => todo!(),
        }
    }

    fn width(&self) -> usize {
        self.framebuffer.info().width
    }

    fn height(&self) -> usize {
        self.framebuffer.info().height
    }
}

struct DisplayLogger;

impl log::Log for DisplayLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= *LOG_LEVEL
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

/// Prints the arguments to the screen, panicking if unable to.
#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    CONSOLE
        .borrow_mut()
        .as_mut()
        .unwrap()
        .write_fmt(args)
        .unwrap();
}

/// Prints the arguments to the console. May panic!.
#[macro_export]
macro_rules! try_print {
    ($($arg:tt)*) => {$crate::sys::_print(format_args!($($arg)*))};
}

/// Prints the arguments to the console and moves to the next line. May panic!.
#[macro_export]
macro_rules! try_println {
    () => ($crate::try_print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

/// Tries to prints the arguments to the screen if the console isn't currently in use.
///
/// May still panic if the console hasn't been initialized.
#[doc(hidden)]
pub fn _try_print(args: core::fmt::Arguments) -> Result<(), BorrowMutError> {
    use core::fmt::Write;
    CONSOLE
        .try_borrow_mut()?
        .as_mut()
        .unwrap()
        .write_fmt(args)
        .unwrap();
    Ok(())
}
