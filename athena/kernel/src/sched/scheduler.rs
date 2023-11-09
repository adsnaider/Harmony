use alloc::sync::Arc;
use core::cell::RefCell;
use core::fmt::Debug;
use core::mem;

use intrusive_collections::{LinkedList, RBTree};

use super::task::{ByTidAdapter, HasContext, ReadyAdapter, Task, TaskHandle, TaskInfo, Tid};
use crate::arch;
use crate::arch::context::Context;

/// The kernel scheduler.
#[derive(Debug)]
pub struct Scheduler {
    current: RefCell<Arc<TaskInfo>>,
    readyq: RefCell<LinkedList<ReadyAdapter>>,
    tasks: RefCell<RBTree<ByTidAdapter>>,
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
    pub fn push(&self, task: Task) -> TaskHandle {
        let task = Arc::new(TaskInfo::new(task));
        self.tasks.borrow_mut().insert(task.clone());
        self.readyq.borrow_mut().push_back(task.clone());
        task.into()
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
        previous
            .exited
            .signal(())
            .expect("Process exited more than once ???");
        if self.tasks.borrow().is_empty() {
            panic!("No more tasks to run :O");
        }

        let next = self.get_next();
        log::debug!("Exiting task: {:?} - Next: {:?}", previous.tid, next.tid);
        *self.current.borrow_mut() = next.clone();
        self.jump_to(next);
    }

    /// Blocks the current process by not placing it in the readyq.
    pub fn block(&self) {
        let next = self.get_next();
        let previous = mem::replace(&mut *self.current.borrow_mut(), next.clone());
        self.switch_to(next, previous)
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
    pub unsafe fn wake_up(&self, task: TaskHandle) {
        self.readyq.borrow_mut().push_back(task.into_info());
    }

    /// Gets the current thread's TID.
    pub fn tid(&self) -> Tid {
        self.current.borrow().tid
    }

    /// Gets the current thread's task information.
    pub fn current(&self) -> TaskHandle {
        self.current.borrow().clone().into()
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
