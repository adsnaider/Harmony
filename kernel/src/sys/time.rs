//! Time utilities (clock, sleep, etc.).

mod timer;

use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;

use futures::Future;

use super::drivers::pit8253::PitTimer;
use super::drivers::Pit8253;
use super::interrupts::async_interrupt::{InterruptCounterCore, InterruptFuture};
use crate::singleton::Singleton;

static TICKS: AtomicU64 = AtomicU64::new(0);
static TIMER: Singleton<PitTimer> = Singleton::uninit();

// Almost 200hz
const PIT_RESET_VALUE: u16 = 5966;
const PIT_FREQ: f32 = PitTimer::freq(PIT_RESET_VALUE);

/// Initializes the PIT timer and returns the internal task that needs to get spawned.
///
/// # Arguments
///
/// * `pit`: A handle to the unique PIT driver.
/// * `timer_future`: The `InterruptFuture` associated with the timer interrupt handler.
pub(super) fn init(
    pit: Pit8253,
    timer_future: InterruptFuture<'static, InterruptCounterCore>,
) -> impl Future<Output = ()> + 'static {
    critical_section::with(|cs| {
        let timer = pit.into_timer(PIT_RESET_VALUE);
        TIMER.initialize(timer, cs);
    });

    tick(timer_future)
}

/// Asynchronous function that continuously updates the internal tick counter.
async fn tick(mut timer_future: InterruptFuture<'static, InterruptCounterCore>) {
    loop {
        let ticks = timer_future.next().await;
        TICKS.fetch_add(ticks as u64, Ordering::Relaxed);
        while let Some(waker) = timer::PENDING_TIMERS.pop() {
            waker.wake();
        }
    }
}

/// Sleeps the entire system for the specified duration.
pub fn sleep_sync(duration: Duration) {
    let start = Instant::now();

    while start.elapsed() < duration {
        x86_64::instructions::hlt();
    }
}

/// Asynchronous sleep for the specified duration.
pub async fn sleep(duration: Duration) {
    timer::Timer::new(duration).await;
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
        Duration::from_secs_f32(diff as f32 / PIT_FREQ)
    }
}
