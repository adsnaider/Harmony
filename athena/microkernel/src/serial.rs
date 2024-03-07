//! Helpers to communicate with the serial port.

use core::cell::RefCell;

use critical_section::Mutex;
use log::{LevelFilter, Metadata, Record};
use once_cell::sync::Lazy;
use uart_16550::SerialPort;

/// Initializes serial port and logger. sprint! and log macros after this.
pub(super) fn init() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(*LOG_LEVEL))
        .expect("Couldn't set the serial logger");
}

static SERIAL: Lazy<Mutex<RefCell<SerialPort>>> = Lazy::new(|| {
    // SAFETY: Serial port address base is correct.
    let mut serial_port = unsafe { SerialPort::new(0x3F8) };
    serial_port.init();
    Mutex::new(RefCell::new(serial_port))
});

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    critical_section::with(|cs| {
        SERIAL
            .borrow_ref_mut(cs)
            .write_fmt(args)
            .expect("Printing to serial failed");
    })
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
        ($($crate::dbg!($val)),+,)
    };
}

/// The global logger.
static LOGGER: Logger = Logger {};

static LOG_LEVEL: Lazy<LevelFilter> = Lazy::new(|| {
    let level = option_env!("KERNEL_LOG_LEVEL").unwrap_or("info");
    match level {
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
