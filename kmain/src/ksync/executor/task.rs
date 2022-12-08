use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::task::Wake;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicU64, Ordering};
use core::task::{Context, Poll, Waker};

use crossbeam::queue::SegQueue;

pub type TaskId = u64;

pub struct Task {
    id: TaskId,
    task: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
}

impl Task {
    pub fn new<F>(task: F) -> Self
    where
        F: Future<Output = ()> + Send + 'static,
    {
        static CURRENT_ID: AtomicU64 = AtomicU64::new(0);
        Self {
            id: CURRENT_ID.fetch_add(1, Ordering::Relaxed),
            task: Box::pin(task),
        }
    }

    fn waker(&self, readyq: &Arc<SegQueue<TaskId>>) -> Waker {
        Arc::new(TaskWaker {
            readyq: Arc::clone(readyq),
            task_id: self.id(),
        })
        .into()
    }

    pub fn poll(&mut self, readyq: &Arc<SegQueue<TaskId>>) -> Poll<()> {
        let waker = self.waker(readyq);
        let mut cx = Context::from_waker(&waker);
        Future::poll(self.task.as_mut(), &mut cx)
    }

    pub fn id(&self) -> TaskId {
        self.id
    }
}

struct TaskWaker {
    readyq: Arc<SegQueue<TaskId>>,
    task_id: TaskId,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.readyq.push(self.task_id);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.readyq.push(self.task_id);
    }
}
