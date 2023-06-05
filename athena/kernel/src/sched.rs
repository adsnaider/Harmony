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
pub fn wakeup(tid: u64) {
    critical_section::with(|cs| SCHEDULER.get().unwrap().borrow(cs).wakeup(tid))
}

impl Scheduler {
    fn next_tid() -> u64 {
        static NEXT_TID: AtomicU64 = AtomicU64::new(0);
        NEXT_TID.fetch_add(1, Ordering::Relaxed)
    }

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
        unsafe { self.switch_to(next, previous) }
    }

    /// Terminates the currently running task and schedules the next one
    pub fn exit(&self) -> ! {
        let previous = self.current.borrow_mut().take().unwrap();
        self.tasks.borrow_mut().remove(&previous).unwrap();

        let next = self.get_next();
        *self.current.borrow_mut() = Some(next);
        unsafe {
            self.jump_to(next);
        }
    }

    /// Blocks the current process
    pub fn block(&self) {
        let previous = self.current.borrow_mut().take().unwrap();
        assert!(self.blocked.borrow_mut().insert(previous));
        let next = self.get_next();
        *self.current.borrow_mut() = Some(next);
        unsafe { self.switch_to(next, previous) }
    }

    /// Awake a blocked context.
    pub fn wakeup(&self, id: u64) {
        if self.blocked.borrow_mut().remove(&id) {
            self.readyq.borrow_mut().push_back(id);
        }
    }

    fn try_get_next(&self) -> Option<u64> {
        self.readyq.borrow_mut().pop_front()
    }

    fn get_next(&self) -> u64 {
        self.try_get_next().unwrap_or(self.looper)
    }

    unsafe fn switch_to(&self, next: u64, previous: u64) {
        assert!(next != previous);
        unsafe {
            let previous = self.tasks.borrow()[&previous].get();
            let next = self.tasks.borrow()[&next].get();

            Context::switch(&*next, &mut *previous);
        }
    }

    unsafe fn jump_to(&self, next: u64) -> ! {
        unsafe {
            let next = self.tasks.borrow()[&next].get();
            Context::jump(&*next);
        }
    }
}
