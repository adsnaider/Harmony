//! Helpers to communicate with the serial port.
#![no_std]

extern crate self as serial;

use log::{LevelFilter, Metadata, Record};
use sync::cell::AtomicLazyCell;
use uart_16550::SerialPort;

/// Initializes serial port and logger. sprint! and log macros after this.
pub fn init() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(*LOG_LEVEL))
        .expect("Couldn't set the serial logger");

    log::info!("Logging initialized");
}

// TODO: Fix this to not use static mut
static mut SERIAL: AtomicLazyCell<SerialPort> = AtomicLazyCell::new(|| {
    // SAFETY: Serial port address base is correct.
    let mut serial_port = unsafe { SerialPort::new(0x3F8) };
    serial_port.init();
    serial_port
});

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    unsafe {
        SERIAL.write_fmt(args).expect("Printing to serial failed");
    }
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! sprint {
    ($($arg:tt)*) => {
        ::serial::_print(core::format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! sprintln {
    () => (::serial::sprint!("\n"));
    ($fmt:expr) => (::serial::sprint!(core::concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (::serial::sprint!(
        core::concat!($fmt, "\n"), $($arg)*));
}

/// Prints a debug expression to the serial port.
#[macro_export]
macro_rules! sdbg {
    () => {
        $crate::sprintln!("[{}:{}]", core::file!(), core::line!())
    };
    ($val:expr $(,)?) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                $crate::sprintln!("[{}:{}] {} = {:#?}",
                    core::file!(), core::line!(), core::stringify!($val), &tmp);
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::sdbg!($val)),+,)
    };
}

/// The global logger.
static LOGGER: Logger = Logger {};

static LOG_LEVEL: AtomicLazyCell<LevelFilter> = AtomicLazyCell::new(|| {
    let level = core::option_env!("RUST_LOG").unwrap_or("info");
    match level {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        other => core::panic!("Unknown LOG LEVEL: {other}"),
    }
});

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= *LOG_LEVEL
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            crate::sprintln!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}
