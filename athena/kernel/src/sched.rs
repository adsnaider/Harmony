//! The kernel scheduler.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::cell::{RefCell};
use core::fmt::Debug;



use critical_section::{CriticalSection, Mutex};


use once_cell::sync::OnceCell;

use crate::arch::context::privileged::KThread;

/// The kernel scheduler.
#[derive(Debug)]
pub struct Scheduler {
    readyq: Mutex<RefCell<VecDeque<KThread>>>,
    blocked: Mutex<RefCell<Vec<KThread>>>,
    current: Mutex<RefCell<Option<KThread>>>,
}

static SCHEDULER: OnceCell<Scheduler> = OnceCell::new();

/// Initializes the scheduler.
pub fn init() {
    SCHEDULER.set(Scheduler::new()).unwrap();
}

/// Pushes a new task to be scheduled.
pub fn push(task: KThread) {
    SCHEDULER.get().unwrap().push(task);
}

/// Performs a context switch.
pub fn switch() {
    SCHEDULER.get().unwrap().switch();
}

/// Starts the scheduler.
pub fn run() -> ! {
    assert!(!crate::arch::int::are_enabled());
    crate::arch::int::enable();
    SCHEDULER.get().unwrap().run();
}

/// Marks the current context as completed.
pub fn kill() -> ! {
    todo!();
}

impl Scheduler {
    /// Creates an empty scheduler.
    pub fn new() -> Self {
        Self {
            readyq: Mutex::new(RefCell::new(VecDeque::new())),
            current: Mutex::new(RefCell::new(None)),
            blocked: Mutex::new(RefCell::new(Vec::new())),
        }
    }

    /// Pushes a new task to the scheduler.
    pub fn push(&self, task: KThread) {
        critical_section::with(|cs| {
            self.readyq.borrow_ref_mut(cs).push_back(task);
        });
    }

    /// Schedules the next task to run.
    ///
    /// Upon a follow up switch, the function will return back to its caller.
    pub fn switch(&self) {
        // Manually disable interrupts as they'll have to be reenabled in the `switch` function.
        assert!(crate::arch::int::are_enabled());
        crate::arch::int::disable();
        // SAFETY: Interrupts are disabled.
        let cs = unsafe { CriticalSection::new() };
        let mut readyq = self.readyq.borrow_ref_mut(cs);
        let mut current = self.current.borrow_ref_mut(cs);
        if let Some(next) = readyq.pop_front() {
            let previous = current.replace(next).unwrap();
            readyq.push_back(previous);
            let previous: *mut KThread = readyq.back_mut().unwrap();
            let next: *const KThread = current.as_ref().unwrap();
            drop(readyq);
            drop(current);
            drop(cs);
            // This function will restore interrupts
            unsafe {
                (*next).switch(previous);
            }
        }
    }

    /// Starts the scheduler.
    pub fn run(&self) -> ! {
        assert!(crate::arch::int::are_enabled());
        crate::arch::int::disable();
        let cs = unsafe { CriticalSection::new() };
        let mut readyq = self.readyq.borrow_ref_mut(cs);
        let mut current = self.current.borrow_ref_mut(cs);
        let next = readyq.pop_front().unwrap();
        let mut dummy = KThread::dummy();
        assert!(current.replace(next).is_none());
        let next: *const KThread = current.as_ref().unwrap();
        drop(readyq);
        drop(current);
        drop(cs);
        unsafe {
            (*next).switch(&mut dummy);
        }
        unreachable!();
    }
}
