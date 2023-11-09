//! A preemptible couroutine that can be executed by the OS scheduler.
mod kthread;
mod uthread;

use alloc::sync::Arc;
use core::num::NonZeroU64;
use core::sync::atomic::{AtomicU64, Ordering};

use enum_dispatch::enum_dispatch;
use intrusive_collections::{
    intrusive_adapter, KeyAdapter, LinkedListAtomicLink, RBTreeAtomicLink,
};

use self::kthread::KThread;
use self::uthread::UThread;

#[enum_dispatch(Task)]
pub(super) trait HasContext {
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

    /// Schedules the task to be executed by the scheduler.
    pub fn schedule(self) -> TaskHandle {
        super::spawn(self)
    }
}

#[derive(Debug)]
pub(super) struct TaskInfo {
    pub tid: Tid,
    pub task: Task,

    // Intrusive links
    ready_link: LinkedListAtomicLink,
    blocked_link: LinkedListAtomicLink,
    by_tid_link: RBTreeAtomicLink,
}

impl TaskInfo {
    pub fn new(task: Task) -> Self {
        Self {
            tid: Tid::make_unique(),
            task,
            ready_link: Default::default(),
            blocked_link: Default::default(),
            by_tid_link: Default::default(),
        }
    }
}

intrusive_adapter!(pub(super) ReadyAdapter = Arc<TaskInfo>: TaskInfo { ready_link: LinkedListAtomicLink });
intrusive_adapter!(pub(super) BlockedAdapter = Arc<TaskInfo>: TaskInfo { blocked_link: LinkedListAtomicLink });
intrusive_adapter!(pub(super) ByTidAdapter = Arc<TaskInfo>: TaskInfo { by_tid_link: RBTreeAtomicLink });

impl<'a> KeyAdapter<'a> for ByTidAdapter {
    type Key = Tid;
    fn get_key(&self, x: &'a TaskInfo) -> Tid {
        x.tid
    }
}

/// A thread identifier.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct Tid(NonZeroU64);

impl Tid {
    /// Returns the next available TID.
    pub(super) fn make_unique() -> Self {
        static NEXT_TID: AtomicU64 = AtomicU64::new(1);
        Self(NonZeroU64::new(NEXT_TID.fetch_add(1, Ordering::Relaxed)).unwrap())
    }

    /// Returns the Tid for the current task.
    pub fn this() -> Tid {
        super::tid()
    }
}

/// A direct handle into a task.
///
/// *Note: Owning this handle will prevent the task inforamtion
/// from being garbage collected.*
#[derive(Debug, Clone)]
pub struct TaskHandle(Arc<TaskInfo>);

impl From<Arc<TaskInfo>> for TaskHandle {
    fn from(value: Arc<TaskInfo>) -> Self {
        Self(value)
    }
}

impl TaskHandle {
    /// Returns the task handle for the currently executing task.
    pub fn this() -> Self {
        super::current()
    }
    /// Wakes up the blocked task.
    pub unsafe fn wake_up(&self) {
        unsafe { super::wake_up(self.clone()) }
    }

    pub(super) fn into_info(self) -> Arc<TaskInfo> {
        self.0
    }
}
