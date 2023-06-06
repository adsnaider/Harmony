//! The humble semaphore that lays the foundation for synchronization.

use alloc::collections::VecDeque;
use core::cell::RefCell;

use critical_section::Mutex;

use crate::sched;

/// A semaphore that blocks the process when the count is 0.
#[derive(Debug)]
pub struct Semaphore {
    count: Mutex<RefCell<i64>>,
    blocked_threads: Mutex<RefCell<VecDeque<u64>>>,
}

impl Semaphore {
    /// Create a new semaphore with a starting value.
    pub fn new(count: i64) -> Self {
        Self {
            count: Mutex::new(RefCell::new(count)),
            blocked_threads: Mutex::new(RefCell::new(VecDeque::new())),
        }
    }

    /// Increments the count, potentially unblocking a thread.
    pub fn signal(&self) {
        critical_section::with(|cs| {
            if let Some(tid) = self.blocked_threads.borrow_ref_mut(cs).pop_front() {
                // SAFETY: `tid` is in `blocked_threads` guarantees the blocked reason is due to semaphore.
                unsafe {
                    sched::wakeup(tid);
                }
            }
            *self.count.borrow_ref_mut(cs) += 1;
        });
    }

    /// Decrements the count, potentially blocking the current thread if the count is 0.
    pub fn wait(&self) {
        critical_section::with(|cs| {
            let sleep = {
                let mut count = self.count.borrow_ref_mut(cs);
                *count -= 1;
                *count < 0
            };
            if sleep {
                self.blocked_threads
                    .borrow_ref_mut(cs)
                    .push_back(sched::tid());
                sched::block();
            }
        });
    }
}
