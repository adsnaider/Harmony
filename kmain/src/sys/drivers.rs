//! Device drivers.

pub mod pit8253;
use core::cell::RefCell;

use critical_section::{CriticalSection, Mutex};

pub use self::pit8253::Pit8253;

struct Devices {
    pit: Option<Pit8253>,
}

static DEVICES: Mutex<RefCell<Devices>> = Mutex::new(RefCell::new(Devices {
    pit: Some(unsafe { Pit8253::new() }),
}));

/// Takes the Intel 8253 handle.
///
/// Note: While safe, this function can deadlock if called within an interrupt handler.
pub fn take_pit(cs: CriticalSection) -> Option<Pit8253> {
    DEVICES.borrow_ref_mut(cs).pit.take()
}
