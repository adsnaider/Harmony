//! Synchronization utilities for preempted-execution.

pub mod mutex;
pub mod sem;
pub mod signal;

pub use mutex::Mutex;
pub use sem::Semaphore;

mod block_list {

    use alloc::sync::Arc;
    use core::cell::RefCell;

    use critical_section::Mutex;
    use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};

    use crate::sched::{self, Tid};

    #[derive(Debug)]
    pub struct BlockedThread {
        link: LinkedListLink,
        tid: Tid,
    }

    // SAFETY: FIXME: ...
    unsafe impl Sync for BlockedThread {}

    impl BlockedThread {
        unsafe fn wakeup(&self) {
            unsafe {
                sched::wakeup(self.tid);
            }
        }

        fn this() -> Self {
            Self {
                tid: sched::tid(),
                link: LinkedListLink::new(),
            }
        }
    }

    intrusive_adapter!(BlockedAdapter = Arc<BlockedThread>: BlockedThread { link: LinkedListLink });

    #[derive(Debug)]
    pub struct BlockQueue {
        blocked: Mutex<RefCell<LinkedList<BlockedAdapter>>>,
    }

    impl BlockQueue {
        pub fn new() -> Self {
            Self {
                blocked: Mutex::new(RefCell::new(LinkedList::new(BlockedAdapter::new()))),
            }
        }

        pub fn awake_single(&self) -> bool {
            let thread = critical_section::with(|cs| self.blocked.borrow_ref_mut(cs).pop_front());
            match thread {
                Some(thread) => unsafe {
                    thread.wakeup();
                    true
                },
                None => false,
            }
        }

        pub fn block_current(&self) {
            let blocked = Arc::new(BlockedThread::this());
            critical_section::with(|cs| {
                self.blocked.borrow_ref_mut(cs).push_back(blocked);
            });
            sched::block();
        }

        pub fn awake_all(&self) {
            critical_section::with(|cs| {
                while let Some(thread) = self.blocked.borrow_ref_mut(cs).pop_front() {
                    unsafe {
                        thread.wakeup();
                    }
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;

    use super::*;
    use crate::sched::{self, Task};

    #[test_case]
    fn synchronization() {
        const THREADS: usize = 10;
        const COUNT: usize = 100000;

        let count = Arc::new(Mutex::new(0));
        let done_threads = Arc::new(Semaphore::new(0));

        for _ in 0..THREADS {
            let done_threads = Arc::clone(&done_threads);
            let count = Arc::clone(&count);
            sched::push(Task::kthread(move || {
                for _ in 0..COUNT {
                    let mut count = count.lock();
                    let cached = *count;
                    sched::switch();
                    *count = cached + 1;
                }
                done_threads.signal();
            }));
        }

        for _ in 0..THREADS {
            done_threads.wait();
        }
        assert_eq!(*count.lock(), THREADS * COUNT);
    }
}
