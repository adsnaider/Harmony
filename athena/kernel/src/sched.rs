//! The kernel scheduler.

mod kthread;
mod uthread;

use alloc::sync::Arc;
use core::cell::RefCell;
use core::fmt::Debug;
use core::mem;
use core::sync::atomic::{AtomicU64, Ordering};

pub use block_queue::BlockQueue;
use critical_section::Mutex;
use enum_dispatch::enum_dispatch;
use intrusive_collections::{
    intrusive_adapter, KeyAdapter, LinkedList, LinkedListAtomicLink, RBTree, RBTreeAtomicLink,
};
use once_cell::sync::OnceCell;

use self::kthread::KThread;
use self::uthread::UThread;
use crate::arch;
use crate::arch::context::Context;

#[enum_dispatch(Task)]
trait HasContext {
    fn context(&self) -> *const crate::arch::context::Context;
    fn context_mut(&self) -> *mut crate::arch::context::Context;
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

#[derive(Debug)]
struct TaskInfo {
    tid: Tid,
    task: Task,

    // Intrusive links
    ready_link: LinkedListAtomicLink,
    blocked_link: LinkedListAtomicLink,
    by_tid_link: RBTreeAtomicLink,
}

impl TaskInfo {
    pub fn new(task: Task) -> Self {
        Self {
            tid: Tid::next(),
            task,
            ready_link: Default::default(),
            blocked_link: Default::default(),
            by_tid_link: Default::default(),
        }
    }
}

intrusive_adapter!(ReadyAdapter = Arc<TaskInfo>: TaskInfo { ready_link: LinkedListAtomicLink });
intrusive_adapter!(ByTidAdapter = Arc<TaskInfo>: TaskInfo { by_tid_link: RBTreeAtomicLink });

impl<'a> KeyAdapter<'a> for ByTidAdapter {
    type Key = Tid;
    fn get_key(&self, x: &'a TaskInfo) -> Tid {
        x.tid
    }
}

/// The kernel scheduler.
#[derive(Debug)]
struct Scheduler {
    current: RefCell<Arc<TaskInfo>>,
    readyq: RefCell<LinkedList<ReadyAdapter>>,
    tasks: RefCell<RBTree<ByTidAdapter>>,
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
fn block() {
    critical_section::with(|cs| SCHEDULER.get().unwrap().borrow(cs).block())
}

/// Wakes up a given context (if blocked).
///
/// # Safety
///
/// Awaking a thread can lead to data races if the thread was blocked due to synchronization, for instance.
/// Calling `wakeup` on a thread should only be done if the blocked reason is known and can be guaranteed
/// that it's safe to awaken the thread.
unsafe fn wakeup(info: Arc<TaskInfo>) {
    // SAFETY: Precondition.
    unsafe { critical_section::with(|cs| SCHEDULER.get().unwrap().borrow(cs).wakeup(info)) }
}

/// Gets the current thread's TID.
pub fn tid() -> Tid {
    critical_section::with(|cs| SCHEDULER.get().unwrap().borrow(cs).tid())
}

fn tinfo() -> Arc<TaskInfo> {
    critical_section::with(|cs| SCHEDULER.get().unwrap().borrow(cs).tinfo())
}

impl Scheduler {
    /// Creates an empty scheduler.
    pub fn new(current: Task) -> Self {
        let current = Arc::new(TaskInfo::new(current));
        let mut tasks = RBTree::new(ByTidAdapter::new());

        tasks.insert(current.clone());

        Self {
            current: RefCell::new(current),
            readyq: Default::default(),
            tasks: RefCell::new(tasks),
        }
    }

    /// Pushes a new task to the scheduler.
    pub fn push(&self, task: Task) -> Tid {
        let task = Arc::new(TaskInfo::new(task));
        let tid = task.tid;
        self.tasks.borrow_mut().insert(task.clone());
        self.readyq.borrow_mut().push_back(task);
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
        let previous = mem::replace(&mut *self.current.borrow_mut(), next.clone());
        log::debug!("Switching to {:?} from {:?}", next.tid, previous.tid);
        self.readyq.borrow_mut().push_back(previous.clone());
        self.switch_to(next, previous)
    }

    /// Terminates the currently running task and schedules the next one
    pub fn exit(&self) -> ! {
        let previous = unsafe {
            self.tasks
                .borrow_mut()
                .cursor_mut_from_ptr(self.current.borrow().as_ref())
                .remove()
                .unwrap()
        };
        if self.tasks.borrow().is_empty() {
            panic!("No more tasks to run :O");
        }

        let next = self.get_next();
        log::debug!("Exiting task: {:?} - Next: {:?}", previous.tid, next.tid);
        *self.current.borrow_mut() = next.clone();
        self.jump_to(next);
    }

    /// Blocks the current process by not placing it in the readyq.
    fn block(&self) {
        let next = self.get_next();
        let previous = mem::replace(&mut *self.current.borrow_mut(), next.clone());
        self.switch_to(next, previous)
    }

    /// Awakes a blocked context.
    unsafe fn wakeup(&self, task: Arc<TaskInfo>) {
        self.readyq.borrow_mut().push_back(task);
    }

    /// Gets the current thread's TID.
    pub fn tid(&self) -> Tid {
        self.current.borrow().tid
    }

    /// Gets the current thread's task information.
    pub fn tinfo(&self) -> Arc<TaskInfo> {
        self.current.borrow().clone()
    }

    fn try_get_next(&self) -> Option<Arc<TaskInfo>> {
        self.readyq.borrow_mut().pop_front()
    }

    fn get_next(&self) -> Arc<TaskInfo> {
        loop {
            match self.try_get_next() {
                Some(tid) => break tid,
                None => arch::inst::hlt(),
            }
        }
    }

    fn switch_to(&self, next: Arc<TaskInfo>, previous: Arc<TaskInfo>) {
        assert!(!Arc::ptr_eq(&next, &previous));
        // SAFETY: All of the tasks in the map are properly initialized and `next` and `previous`
        // are not the same.
        unsafe {
            let previous = previous.task.context_mut();
            let next = next.task.context();

            Context::switch(next, previous);
        }
    }

    fn jump_to(&self, next: Arc<TaskInfo>) -> ! {
        // SAFETY: All of the tasks in the map are properly initialized.
        unsafe {
            let next = next.task.context();
            Context::jump(next);
        }
    }
}

mod block_queue {
    use alloc::sync::Arc;
    use core::cell::RefCell;

    use critical_section::Mutex;
    use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListAtomicLink};

    use crate::sched::TaskInfo;

    intrusive_adapter!(BlockedAdapter = Arc<TaskInfo>: TaskInfo { blocked_link: LinkedListAtomicLink });

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
            thread.map(|th| unsafe { super::wakeup(th) }).is_some()
        }

        pub fn block_current(&self) {
            let blocked = super::tinfo();
            critical_section::with(|cs| {
                self.blocked.borrow_ref_mut(cs).push_back(blocked);
            });
            super::block();
        }

        pub fn awake_all(&self) {
            critical_section::with(|cs| {
                while let Some(thread) = self.blocked.borrow_ref_mut(cs).pop_front() {
                    unsafe {
                        super::wakeup(thread);
                    }
                }
            })
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
