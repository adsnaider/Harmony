//! A signal that wakes up all blocked threads when set.

use core::cell::RefCell;

use critical_section::Mutex;

use crate::sched::BlockQueue;

/// A synchronization primitive that wakes up all blocked threads once a signal is set.
#[derive(Debug)]
pub struct Signal {
    set: Mutex<RefCell<bool>>,
    blocked: BlockQueue,
}

impl Default for Signal {
    fn default() -> Self {
        Self::new()
    }
}

impl Signal {
    /// Makes a new unset signal.
    pub fn new() -> Self {
        Self {
            set: Mutex::new(RefCell::new(false)),
            blocked: BlockQueue::new(),
        }
    }

    /// Blocks this thread until the signal has been set.
    pub fn wait(&self) {
        critical_section::with(|cs| {
            if !(*self.set.borrow_ref(cs)) {
                self.blocked.block_current()
            }
        })
    }

    /// Wakes up all blocked threads and prevents further threads from blocking on [`Signal::wait`].
    pub fn signal(&self) {
        critical_section::with(|cs| {
            *self.set.borrow_ref_mut(cs) = true;
            self.blocked.awake_all();
        })
    }
}
