//! Device drivers.

use spin::Mutex;

pub mod pit8253;
pub use self::pit8253::Pit8253;

struct Devices {
    pit: Option<Pit8253>,
}

static DEVICES: Mutex<Devices> = Mutex::new(Devices {
    pit: Some(unsafe { Pit8253::new() }),
});

/// Takes the Intel 8253 handle.
///
/// Note: While safe, this function can deadlock if called within an interrupt handler.
pub fn take_pit() -> Option<Pit8253> {
    DEVICES.lock().pit.take()
}
