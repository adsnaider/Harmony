//! A signal that wakes up all blocked threads when set.

use once_cell::sync::OnceCell;

use crate::sched::BlockQueue;

/// A synchronization primitive that wakes up all blocked threads once a signal is set.
#[derive(Debug)]
pub struct Signal<T> {
    value: OnceCell<T>,
    blocked: BlockQueue,
}

impl<T> Default for Signal<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Signal<T> {
    /// Makes a new unset signal.
    pub fn new() -> Self {
        Self {
            value: OnceCell::new(),
            blocked: BlockQueue::new(),
        }
    }

    /// Blocks this thread until the signal has been set.
    pub fn wait(&self) -> &T {
        critical_section::with(|_cs| {
            if self.value.get().is_none() {
                self.blocked.block_current();
            }
        });
        self.value.get().unwrap()
    }

    /// Wakes up all blocked threads and prevents further threads from blocking on [`Signal::wait`].
    pub fn signal(&self, value: T) -> Result<(), T> {
        let result = self.value.set(value);
        if result.is_ok() {
            self.blocked.awake_all()
        }
        result
    }

    /// Tries to get the signaled value without blocking.
    pub fn get_value(&self) -> Option<&T> {
        self.value.get()
    }
}
