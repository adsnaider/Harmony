//! Time utilities (clock, sleep, etc.).

use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;

use x86_64::instructions::interrupts::without_interrupts;

use super::drivers::pit8253::PitTimer;
use super::drivers::Pit8253;
use crate::singleton::Singleton;

static TICKS: AtomicU64 = AtomicU64::new(0);
static TIMER: Singleton<PitTimer> = Singleton::uninit();

/// Initializes the time module.
pub(super) fn init(pit: Pit8253) {
    without_interrupts(|| {
        // Initialize timer to almost 200hz
        let timer = pit.into_timer(5966);
        TIMER.initialize(timer);
    });
}

/// Increments the internal tick counter.
pub(super) fn tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

/// Sleeps the entire system for the specified duration.
pub fn sleep_sync(duration: Duration) {
    let start = Instant::now();

    while start.elapsed() < duration {
        x86_64::instructions::hlt();
    }
}

/// Represents a point in time during execution.
///
/// The value of an instant is only useful in terms of relative differences (i.e. Duration), since
/// the absolute value holds no meaning.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Instant {
    tick: u64,
}

impl Instant {
    /// Returns an instant corresponding to `now`.
    pub fn now() -> Instant {
        let tick = TICKS.load(Ordering::Relaxed);
        Instant { tick }
    }

    /// Returns the amount of time that has elapsed since this instant was created.
    pub fn elapsed(&self) -> Duration {
        let now = Self::now();
        if now.tick < self.tick {
            return Duration::ZERO;
        }
        let diff = now.tick - self.tick;
        Duration::from_secs_f32(diff as f32 / TIMER.lock().freq())
    }
}
