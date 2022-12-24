//! UEFI I/O utilities.

use core::fmt::Write;

use bootinfo::{Framebuffer, PixelBitmask, PixelFormat};
use log::{Metadata, Record};
use uefi::proto::console::gop::GraphicsOutput;

use super::GlobalTable;
use crate::sys::SYSTEM_TABLE;

/// Retrieves the framebuffer. The framebuffer can be used after exiting boot services.
pub fn get_framebuffer() -> Framebuffer {
    let (framebuffer, mode) = {
        let table = SYSTEM_TABLE.get();
        let mut gop = GlobalTable::open_protocol::<GraphicsOutput>(&table)
            .expect("Unable to open GraphicsOutput protocol");

        let framebuffer = gop.frame_buffer().as_mut_ptr();
        let mode = gop.current_mode_info();
        (framebuffer, mode)
    };

    Framebuffer {
        address: framebuffer,
        resolution: mode.resolution(),
        pixel_format: match mode.pixel_format() {
            uefi::proto::console::gop::PixelFormat::Rgb => PixelFormat::Rgb,
            uefi::proto::console::gop::PixelFormat::Bgr => PixelFormat::Bgr,
            uefi::proto::console::gop::PixelFormat::Bitmask => PixelFormat::Bitmask({
                let bitmask = mode
                    .pixel_bitmask()
                    .expect("Bitmask should be set when pixel format is bitmask.");
                PixelBitmask {
                    red: bitmask.red,
                    green: bitmask.green,
                    blue: bitmask.blue,
                    reserved: bitmask.reserved,
                }
            }),
            uefi::proto::console::gop::PixelFormat::BltOnly => PixelFormat::BltOnly,
        },
        stride: mode.stride(),
    }
}

struct UefiLogger;
/// UEFI logger.
static UEFI_LOGGER: UefiLogger = UefiLogger;

/// Initializes logging services. This is to be called by the system after setting up the
/// SYSTEM_TABLE.
pub(super) fn init() {
    log::set_logger(&UEFI_LOGGER)
        .map(|()| log::set_max_level(log::LevelFilter::Info))
        .expect("Couldn't initialize logging services.");
}

impl log::Log for UefiLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        {
            SYSTEM_TABLE.is_set()
        }
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            {
                writeln!(
                    SYSTEM_TABLE.get_mut().stdout(),
                    "{} - {}",
                    record.level(),
                    record.args()
                )
                .expect("Unable to log to screen");
            }
        }
    }

    fn flush(&self) {}
}
