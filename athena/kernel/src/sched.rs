//! The kernel scheduler.

mod kthread;
mod uthread;
use alloc::collections::VecDeque;
use core::cell::RefCell;
use core::fmt::Debug;
use core::sync::atomic::{AtomicU64, Ordering};

use critical_section::Mutex;
use enum_dispatch::enum_dispatch;
use hashbrown::{HashMap, HashSet};
use once_cell::sync::OnceCell;

use self::kthread::KThread;
use self::uthread::UThread;
use crate::arch;
use crate::arch::context::Context;

#[enum_dispatch(Task)]
trait HasContext {
    fn context(&self) -> *const crate::arch::context::Context;
    fn context_mut(&mut self) -> *mut crate::arch::context::Context;
}

/// A runnable task such as a KThread, UThread, etc.
#[enum_dispatch]
#[derive(Debug)]
pub enum Task {
    /// A kernel thread task.
    KThread,
    /// A user thread task.
    UThread,
}

impl Task {
    /// Constructs a kernel thread task.
    pub fn kthread<F>(f: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        Self::KThread(KThread::new(f))
    }

    /// Constructs a user thread with the given program.
    pub fn uthread(program: &[u8]) -> Option<Self> {
        Some(Self::UThread(UThread::new(program)?))
    }
}

/// The kernel scheduler.
#[derive(Debug)]
pub struct Scheduler {
    readyq: RefCell<VecDeque<Tid>>,
    blocked: RefCell<HashSet<Tid>>,
    current: RefCell<Option<Tid>>,
    tasks: RefCell<HashMap<Tid, Task>>,
}

static SCHEDULER: OnceCell<Mutex<Scheduler>> = OnceCell::new();

/// A thread ID.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct Tid(u64);

impl Tid {
    /// Returns the next available TID.
    pub fn next() -> Self {
        static NEXT_TID: AtomicU64 = AtomicU64::new(0);
        Self(NEXT_TID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Initializes the scheduler.
///
/// # Panics
///
/// The main thread (that calls this), **should not** call [`switch`] since
/// the scheduler can't go back to the main thread. Instead, it will place a
/// a thread that panics in its place.
pub fn init() {
    SCHEDULER
        .set(Mutex::new(Scheduler::new(Task::kthread(|| {
            panic!("Main thread called sched::switch. Should call sched::exit instead");
        }))))
        .unwrap();
}

/// Pushes a new task to be scheduled.
pub fn push(task: Task) -> Tid {
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
pub unsafe fn wakeup(tid: Tid) {
    // SAFETY: Precondition.
    unsafe { critical_section::with(|cs| SCHEDULER.get().unwrap().borrow(cs).wakeup(tid)) }
}

/// Gets the current thread's TID.
pub fn tid() -> Tid {
    critical_section::with(|cs| SCHEDULER.get().unwrap().borrow(cs).tid())
}

impl Scheduler {
    /// Creates an empty scheduler.
    pub fn new(current: Task) -> Self {
        let tid = Tid::next();
        let mut tasks = HashMap::new();
        tasks.try_insert(tid, current).unwrap();

        Self {
            readyq: RefCell::new(VecDeque::new()),
            current: RefCell::new(Some(tid)),
            blocked: RefCell::new(HashSet::new()),
            tasks: RefCell::new(tasks),
        }
    }

    /// Pushes a new task to the scheduler.
    pub fn push(&self, task: Task) -> Tid {
        let tid = Tid::next();
        self.tasks.borrow_mut().try_insert(tid, task).unwrap();
        self.readyq.borrow_mut().push_back(tid);
        tid
    }

    /// Schedules the next task to run.
    ///
    /// Upon a follow up switch, the function will return back to its caller.
    pub fn switch(&self) {
        let Some(next) = self.try_get_next() else {
            log::debug!("Nothing else, back to the caller");
            // Nothing else to run, back to caller.
            return;
        };
        let previous = self.current.borrow_mut().replace(next).unwrap();
        log::debug!("Switching to {next:?} from {previous:?}");
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
        if self.tasks.borrow().is_empty() {
            panic!("No more tasks to run :O");
        }

        let next = self.get_next();
        log::debug!("Exiting task: {previous:?} - Next: {next:?}");
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
    pub unsafe fn wakeup(&self, id: Tid) {
        if self.blocked.borrow_mut().remove(&id) {
            self.readyq.borrow_mut().push_back(id);
        }
    }

    /// Gets the current thread's TID.
    pub fn tid(&self) -> Tid {
        self.current.borrow().unwrap()
    }

    fn try_get_next(&self) -> Option<Tid> {
        self.readyq.borrow_mut().pop_front()
    }

    fn get_next(&self) -> Tid {
        loop {
            match self.try_get_next() {
                Some(tid) => break tid,
                None => arch::inst::hlt(),
            }
        }
    }

    fn switch_to(&self, next: Tid, previous: Tid) {
        assert!(next != previous);
        // SAFETY: All of the tasks in the map are properly initialized and `next` and `previous`
        // are not the same.
        unsafe {
            let previous = self
                .tasks
                .borrow_mut()
                .get_mut(&previous)
                .unwrap()
                .context_mut();
            let next = self
                .tasks
                .borrow_mut()
                .get_mut(&next)
                .unwrap()
                .context_mut();

            Context::switch(next, previous);
        }
    }

    fn jump_to(&self, next: Tid) -> ! {
        // SAFETY: All of the tasks in the map are properly initialized.
        unsafe {
            let next = self
                .tasks
                .borrow_mut()
                .get_mut(&next)
                .unwrap()
                .context_mut();
            Context::jump(next);
        }
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
            sched::push(Task::kthread(move || {
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
