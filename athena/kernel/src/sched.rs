//! The kernel scheduler.

pub mod block_queue;
pub mod task;

mod scheduler;
use critical_section::Mutex;
use once_cell::sync::OnceCell;

pub use self::block_queue::BlockQueue;
use self::scheduler::Scheduler;
pub use self::task::{Task, TaskHandle, Tid};

static SCHEDULER: OnceCell<Mutex<Scheduler>> = OnceCell::new();

fn with_scheduler<T, F: FnOnce(&Scheduler) -> T>(fun: F) -> T {
    critical_section::with(|cs| fun(SCHEDULER.get().unwrap().borrow(cs)))
}

/// Initializes the scheduler.
///
/// # Panics
///
/// The main thread (that calls this), **should not** call [`yield_now`] since
/// the scheduler can't go back to the main thread. Instead, it will place a
/// a thread that panics in its place.
pub fn init() {
    SCHEDULER
        .set(Mutex::new(Scheduler::new(Task::kthread(|| {
            panic!("Main thread called sched::switch. Should call sched::exit instead");
        }))))
        .unwrap();
}

/// Schedules the task to be executed by the scheduler.
pub fn spawn(task: Task) -> TaskHandle {
    with_scheduler(|sched| sched.push(task))
}

/// Yields the thread of execution to the next thread.
pub fn yield_now() {
    with_scheduler(|sched| sched.switch())
}

/// Terminates the currently running task.
pub fn exit() -> ! {
    with_scheduler(|sched| sched.exit())
}

/// Blocks the current context until a wake up signal is received.
pub fn block() {
    with_scheduler(|sched| sched.block())
}

/// Wakes up the given task.
///
/// Only reason this should be called is to build up a concurrency primitive,
/// however, `[BlockQueue]` is an even lower level primitive that may be used
/// safely build other primitives.
///
/// # Safety
///
/// * The task must be blocked (i.e. it called `[block]`).
/// * This call to `[wake_up]` is directly associated with an earlier call to `[block]`.
///
/// In broad terms, a task may be blocked for 1 "reason" at a time. Whatever the reason for this
/// was, whoever made the decision to block the task is the one that may unblock it. The reason
/// for this is essentially that block/wake up calls will be used to build up synchronization
/// primitives and no thread safety guarantees could be made if anyone may wake up a task at any
/// time.
pub unsafe fn wake_up(handle: TaskHandle) {
    // SAFETY: Precondition.
    unsafe { with_scheduler(|sched| sched.wake_up(handle)) }
}

/// Gets the current thread's TID.
pub fn tid() -> Tid {
    with_scheduler(|sched| sched.tid())
}

/// Returns the task handle for the currently executing task.
pub fn current() -> TaskHandle {
    with_scheduler(|sched| sched.current())
}

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::sched;
    use crate::sync::Semaphore;

    #[test_case]
    fn threads_run() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let done_threads = Arc::new(Semaphore::new(0));

        const THREADS: usize = 1000;
        const COUNT: usize = 10000;

        for _ in 0..THREADS {
            let done_threads = Arc::clone(&done_threads);
            sched::spawn(Task::kthread(move || {
                for _ in 0..COUNT {
                    COUNTER.fetch_add(1, Ordering::Release);
                }
                done_threads.signal();
            }));
        }

        for _ in 0..THREADS {
            done_threads.wait();
        }

        assert_eq!(COUNTER.load(Ordering::Acquire), THREADS * COUNT);
    }
}
