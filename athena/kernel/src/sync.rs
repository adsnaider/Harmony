//! Synchronization utilities for preempted-execution.

pub mod mutex;
pub mod sem;
pub mod signal;

pub use mutex::Mutex;
pub use sem::Semaphore;

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
            sched::spawn(Task::kthread(move || {
                for _ in 0..COUNT {
                    let mut count = count.lock();
                    let cached = *count;
                    sched::yield_now();
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
