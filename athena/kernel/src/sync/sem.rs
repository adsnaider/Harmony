//! The humble semaphore that lays the foundation for synchronization.

use core::cell::RefCell;

use critical_section::Mutex;

use crate::sched::BlockQueue;

/// A semaphore that blocks the process when the count is 0.
#[derive(Debug)]
pub struct Semaphore {
    count: Mutex<RefCell<i64>>,
    blocked_threads: BlockQueue,
}

impl Semaphore {
    /// Create a new semaphore with a starting value.
    pub fn new(count: i64) -> Self {
        Self {
            count: Mutex::new(RefCell::new(count)),
            blocked_threads: BlockQueue::new(),
        }
    }

    /// Increments the count, potentially unblocking a thread.
    pub fn signal(&self) {
        critical_section::with(|cs| {
            self.blocked_threads.awake_one();
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
                self.blocked_threads.block_current()
            }
        });
    }
}
