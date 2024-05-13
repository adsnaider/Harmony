//! Helpers to communicate with the serial port.

use log::{LevelFilter, Metadata, Record};
use sync::cell::AtomicLazyCell;
use uart_16550::SerialPort;

/// Initializes serial port and logger. sprint! and log macros after this.
pub(super) fn init() {
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
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! sprintln {
    () => ($crate::sprint!("\n"));
    ($fmt:expr) => ($crate::sprint!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::sprint!(
        concat!($fmt, "\n"), $($arg)*));
}

/// Prints a debug expression to the serial port.
#[macro_export]
macro_rules! sdbg {
    () => {
        $crate::sprintln!("[{}:{}]", file!(), line!())
    };
    ($val:expr $(,)?) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                $crate::sprintln!("[{}:{}] {} = {:#?}",
                    file!(), line!(), stringify!($val), &tmp);
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
    let level = option_env!("KERNEL_LOG_LEVEL").unwrap_or("info");
    match level {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        other => panic!("Unknown LOG LEVEL: {other}"),
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
