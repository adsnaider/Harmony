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

intrusive_adapter!(ReadyAdapter = Arc<TaskInfo>: TaskInfo { ready_link: LinkedListAtomicLink });
intrusive_adapter!(BlockedAdapter = Arc<TaskInfo>: TaskInfo { blocked_link: LinkedListAtomicLink });
intrusive_adapter!(ByTidAdapter = Arc<TaskInfo>: TaskInfo { by_tid_link: RBTreeAtomicLink });

impl<'a> KeyAdapter<'a> for ByTidAdapter {
    type Key = Tid;
    fn get_key(&self, x: &'a TaskInfo) -> Tid {
        x.tid
    }
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
