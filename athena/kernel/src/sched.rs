//! The kernel scheduler.

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::fmt::Debug;
use core::sync::atomic::{AtomicU64, Ordering};

use critical_section::Mutex;
use once_cell::sync::OnceCell;

use crate::arch::context::Context;

#[derive(Debug)]
struct SchedContext {
    context: Context,
    id: u64,
}

impl SchedContext {
    pub fn new(context: crate::arch::context::Context) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Self { context, id }
    }
}

impl From<Context> for SchedContext {
    fn from(value: crate::arch::context::Context) -> Self {
        Self::new(value)
    }
}

/// The kernel scheduler.
#[allow(missing_debug_implementations)]
pub struct Scheduler {
    readyq: VecDeque<SchedContext>,
    blocked: Vec<(SchedContext, Box<dyn Fn() -> bool + Send>)>,
    current: SchedContext,
}

static SCHEDULER: OnceCell<Mutex<UnsafeCell<Scheduler>>> = OnceCell::new();

/// Initializes the scheduler.
pub fn init() {
    SCHEDULER
        .set(Mutex::new(UnsafeCell::new(Scheduler::new(Context::main()))))
        .unwrap();
}

/// Pushes a new task to be scheduled.
pub fn push(task: crate::arch::context::Context) {
    critical_section::with(|cs| unsafe {
        (*SCHEDULER.get().unwrap().borrow(cs).get()).push(task);
    })
}

/// Performs a context switch.
pub fn switch() {
    critical_section::with(|cs| unsafe {
        (*SCHEDULER.get().unwrap().borrow(cs).get()).switch();
    })
}

/// Terminates the currently running task and schedules the next one
pub fn exit() -> ! {
    critical_section::with(|cs| unsafe {
        (*SCHEDULER.get().unwrap().borrow(cs).get()).exit();
    })
}

impl Scheduler {
    /// Creates an empty scheduler.
    pub fn new(current: Context) -> Self {
        let mut this = Self {
            readyq: VecDeque::new(),
            current: current.into(),
            blocked: Vec::new(),
        };
        this.push(Context::kthread(Self::looper));
        this
    }

    /// Pushes a new task to the scheduler.
    pub fn push(&mut self, task: Context) {
        self.readyq.push_back(task.into());
    }

    /// Schedules the next task to run.
    ///
    /// Upon a follow up switch, the function will return back to its caller.
    pub fn switch(&mut self) {
        let Some(next) = self
            .readyq
            .pop_front() else {
            // Nothing else to run, back to caller.
            return;
        };
        let previous = core::mem::replace(&mut self.current, next);
        self.readyq.push_back(previous);
        let previous = self.readyq.back_mut().unwrap();
        // SAFETY: This is super awkward but hopefully safe.
        // * It's probably not cool to keep references that need to live after the switch, so we use raw pointers.
        // * The pointers only become references before the switch, not after.
        // * When the switch comes back to us (on a further restore, we don't have any more references around).
        unsafe {
            // This function will restore interrupts
            Context::switch(&self.current.context, &mut previous.context);
        }
    }

    /// Terminates the currently running task and schedules the next one
    pub fn exit(&mut self) -> ! {
        let next = self.readyq.pop_front().expect("Looper should be ready");
        self.current = next;
        unsafe {
            Context::jump(&self.current.context);
        }
    }

    /// Blocks the current process
    pub fn block<F>(&mut self, ready: F)
    where
        F: Fn() -> bool + Send + 'static,
    {
        if !ready() {
            let next = self.readyq.pop_front().expect("Looper should be ready");
            let previous = core::mem::replace(&mut self.current, next);
            self.blocked.push((previous, Box::new(ready)));
            let previous = &mut self.blocked.last_mut().unwrap().0;
            unsafe {
                Context::switch(&self.current.context, &mut previous.context);
            }
        }
    }

    fn looper() {
        loop {
            crate::arch::inst::hlt();
        }
    }

    /// Poll blocked process and awake if ready.
    pub fn poll_blocked(&mut self, id: u64) -> bool {
        todo!();
    }
}
