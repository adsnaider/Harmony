//! The kernel scheduler.

use alloc::collections::VecDeque;
use core::cell::{RefCell, UnsafeCell};
use core::fmt::Debug;
use core::sync::atomic::{AtomicU64, Ordering};

use critical_section::Mutex;
use hashbrown::{HashMap, HashSet};
use once_cell::sync::OnceCell;

use crate::arch::context::Context;

/// The kernel scheduler.
#[derive(Debug)]
pub struct Scheduler {
    readyq: RefCell<VecDeque<u64>>,
    blocked: RefCell<HashSet<u64>>,
    current: RefCell<Option<u64>>,
    tasks: RefCell<HashMap<u64, UnsafeCell<Context>>>,
    looper: u64,
}

static SCHEDULER: OnceCell<Mutex<Scheduler>> = OnceCell::new();

/// Initializes the scheduler.
pub fn init() {
    SCHEDULER
        .set(Mutex::new(Scheduler::new(Context::main())))
        .unwrap();
}

/// Pushes a new task to be scheduled.
pub fn push(task: crate::arch::context::Context) -> u64 {
    critical_section::with(|cs| SCHEDULER.get().unwrap().borrow(cs).push(task))
}

/// Performs a context switch.
pub fn switch() {
    critical_section::with(|cs| SCHEDULER.get().unwrap().borrow(cs).switch())
}

/// Terminates the currently running task and schedules the next one
pub fn exit() -> ! {
    critical_section::with(|cs| SCHEDULER.get().unwrap().borrow(cs).exit())
}

/// Blocks the current context until a wake up is received.
pub fn block() {
    critical_section::with(|cs| SCHEDULER.get().unwrap().borrow(cs).block())
}

/// Wakes up a given context (if blocked).
///
/// # Safety
///
/// Awaking a thread can lead to data races if the thread was blocked due to synchronization, for instance.
/// Calling `wakeup` on a thread should only be done if the blocked reason is known and can be guaranteed
/// that it's safe to awaken the thread.
pub unsafe fn wakeup(tid: u64) {
    // SAFETY: Precondition.
    unsafe { critical_section::with(|cs| SCHEDULER.get().unwrap().borrow(cs).wakeup(tid)) }
}

/// Gets the current thread's TID.
pub fn tid() -> u64 {
    critical_section::with(|cs| SCHEDULER.get().unwrap().borrow(cs).tid())
}

impl Scheduler {
    /// Creates an empty scheduler.
    pub fn new(current: Context) -> Self {
        let tid = Self::next_tid();
        let mut tasks = HashMap::new();
        tasks.try_insert(tid, UnsafeCell::new(current)).unwrap();

        // A fake thread that never ends. Makes it easy to always have something to schedule.
        let looper = Context::kthread(|| loop {
            crate::arch::inst::hlt();
        });
        let looper_id = Self::next_tid();
        tasks
            .try_insert(looper_id, UnsafeCell::new(looper))
            .unwrap();

        Self {
            readyq: RefCell::new(VecDeque::new()),
            current: RefCell::new(Some(tid)),
            blocked: RefCell::new(HashSet::new()),
            tasks: RefCell::new(tasks),
            looper: looper_id,
        }
    }

    /// Pushes a new task to the scheduler.
    pub fn push(&self, task: Context) -> u64 {
        let tid = Self::next_tid();
        self.tasks
            .borrow_mut()
            .try_insert(tid, UnsafeCell::new(task))
            .unwrap();
        self.readyq.borrow_mut().push_back(tid);
        tid
    }

    /// Schedules the next task to run.
    ///
    /// Upon a follow up switch, the function will return back to its caller.
    pub fn switch(&self) {
        let Some(next) = self.try_get_next() else {
            // Nothing else to run, back to caller.
            return;
        };
        let previous = self.current.borrow_mut().replace(next).unwrap();
        self.readyq.borrow_mut().push_back(previous);
        // SAFETY: This is super awkward but hopefully safe.
        // * It's probably not cool to keep references that need to live after the switch, so we use raw pointers.
        // * The pointers only become references before the switch, not after.
        // * When the switch comes back to us (on a further restore, we don't have any more references around).
        self.switch_to(next, previous)
    }

    /// Terminates the currently running task and schedules the next one
    pub fn exit(&self) -> ! {
        let previous = self.current.borrow_mut().take().unwrap();
        self.tasks.borrow_mut().remove(&previous).unwrap();

        let next = self.get_next();
        *self.current.borrow_mut() = Some(next);
        self.jump_to(next);
    }

    /// Blocks the current process
    pub fn block(&self) {
        let previous = self.current.borrow_mut().take().unwrap();
        assert!(self.blocked.borrow_mut().insert(previous));
        let next = self.get_next();
        *self.current.borrow_mut() = Some(next);
        self.switch_to(next, previous)
    }

    /// Awake a blocked context.
    ///
    /// # Safety
    ///
    /// Awaking a thread can lead to data races if the thread was blocked due to synchronization, for instance.
    /// Calling `wakeup` on a thread should only be done if the blocked reason is known and can be guaranteed
    /// that it's safe to awaken the thread.
    pub unsafe fn wakeup(&self, id: u64) {
        if self.blocked.borrow_mut().remove(&id) {
            self.readyq.borrow_mut().push_back(id);
        }
    }

    /// Gets the current thread's TID.
    pub fn tid(&self) -> u64 {
        self.current.borrow().unwrap()
    }

    fn try_get_next(&self) -> Option<u64> {
        self.readyq.borrow_mut().pop_front()
    }

    fn get_next(&self) -> u64 {
        self.try_get_next().unwrap_or(self.looper)
    }

    fn switch_to(&self, next: u64, previous: u64) {
        assert!(next != previous);
        // SAFETY: All of the tasks in the map are properly initialized and `next` and `previous`
        // are not the same.
        unsafe {
            let previous = self.tasks.borrow()[&previous].get();
            let next = self.tasks.borrow()[&next].get();

            Context::switch(next, previous);
        }
    }

    fn jump_to(&self, next: u64) -> ! {
        // SAFETY: All of the tasks in the map are properly initialized.
        unsafe {
            let next = self.tasks.borrow()[&next].get();
            Context::jump(next);
        }
    }

    fn next_tid() -> u64 {
        static NEXT_TID: AtomicU64 = AtomicU64::new(0);
        NEXT_TID.fetch_add(1, Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;
    use core::sync::atomic::AtomicUsize;

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
            sched::push(Context::kthread(move || {
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
