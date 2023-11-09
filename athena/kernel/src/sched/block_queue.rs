//! An API to build concurrency primitives by taking advantage of the scheduler.
//!
//! The basic idea is to build synchronization by blocking and unblocking tasks instead of
//! spinning. The [`BlockQueue`] provides a safe API to do just that
use core::cell::RefCell;

use critical_section::Mutex;
use intrusive_collections::LinkedList;

use super::task::BlockedAdapter;
use super::TaskHandle;

/// A safe API to block/unblock tasks.
///
/// The queue won't perform any allocations when blocking/unblocking tasks. Instead it uses
/// intrusive links in the task information structure to efficiently push and pop tasks on and
/// off the queue.
#[derive(Debug)]
pub struct BlockQueue {
    blocked: Mutex<RefCell<LinkedList<BlockedAdapter>>>,
}

impl BlockQueue {
    /// Makes an empty [`BlockQueue`].
    pub fn new() -> Self {
        Self {
            blocked: Mutex::new(RefCell::new(LinkedList::new(BlockedAdapter::new()))),
        }
    }

    /// Wakes up a single task based on a FIFO ordering.
    pub fn awake_one(&self) -> bool {
        let thread = critical_section::with(|cs| self.blocked.borrow_ref_mut(cs).pop_front());
        thread
            // SAFETY: Task was in the queue so it must have been blocked.
            .map(|th| unsafe { super::wake_up(th.into()) })
            .is_some()
    }

    /// Blocks the current thread of execution.
    pub fn block_current(&self) {
        let this_task = TaskHandle::this();
        critical_section::with(|cs| {
            self.blocked
                .borrow_ref_mut(cs)
                .push_back(this_task.into_info());
            super::block();
        });
    }

    /// Wakes up all of the threads, clearing the queue.
    pub fn awake_all(&self) {
        critical_section::with(|cs| {
            while let Some(thread) = self.blocked.borrow_ref_mut(cs).pop_front() {
                // SAFETY: Task was in the queue so it must have been blocked.
                unsafe {
                    super::wake_up(thread.into());
                }
            }
        })
    }
}

impl Default for BlockQueue {
    fn default() -> Self {
        Self::new()
    }
}
