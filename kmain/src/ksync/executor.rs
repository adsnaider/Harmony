//! The actual executor that runs the futures.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::future::Future;

use crossbeam::queue::SegQueue;

mod task;
use self::task::{Task, TaskId};

/// An async executor suitable for the kernel.
#[allow(missing_debug_implementations)]
pub struct Executor {
    ready: Arc<SegQueue<TaskId>>,
    tasks: BTreeMap<TaskId, Task>,
}

impl Executor {
    /// Constructs a new async executor.
    pub fn new() -> Self {
        Self {
            ready: Arc::new(SegQueue::new()),
            tasks: BTreeMap::new(),
        }
    }

    /// Spawns a new task.
    pub fn spawn<F>(&mut self, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let task = Task::new(task);
        critical_section::with(|_cs| {
            let tid = task.id();
            self.ready.push(tid);
            assert!(
                self.tasks.insert(task.id(), task).is_none(),
                "Task with same ID: {tid} already inserted",
            );
        })
    }

    /// Start the runtime.
    pub fn start(&mut self) -> ! {
        loop {
            while let Some(task_id) = self.ready.pop() {
                let task = self
                    .tasks
                    .get_mut(&task_id)
                    .expect("Task ID {task_id} missing task in b-tree");
                let _ = task.poll(&self.ready);
            }
            x86_64::instructions::hlt();
        }
    }
}
