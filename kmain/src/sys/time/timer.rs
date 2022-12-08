use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use core::time::Duration;

use crossbeam::queue::SegQueue;

use super::Instant;

pub(super) static PENDING_TIMERS: SegQueue<Waker> = SegQueue::new();

/// Asynchronous timer.
pub struct Timer {
    start: Instant,
    duration: Duration,
}

impl Timer {
    /// Construct a timer with the given duration.
    pub fn new(duration: Duration) -> Self {
        Self {
            start: Instant::now(),
            duration,
        }
    }
}

impl Future for Timer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.start.elapsed() >= self.duration {
            Poll::Ready(())
        } else {
            PENDING_TIMERS.push(cx.waker().clone());
            Poll::Pending
        }
    }
}
