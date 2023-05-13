//! The kernel scheduler.

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use core::cell::UnsafeCell;
use core::fmt::Debug;

use critical_section::{CriticalSection, Mutex, RestoreState};

use crate::arch::context::Context;

/// The kernel scheduler.
pub struct Scheduler {
    readyq: VecDeque<Box<dyn Context + Send>>,
    current: Option<Box<dyn Context + Send>>,
}

static SCHEDULER: Mutex<UnsafeCell<Option<Scheduler>>> = Mutex::new(UnsafeCell::new(None));

/// Initializes the scheduler.
pub fn init(cs: CriticalSection) {
    unsafe {
        let scheduler = SCHEDULER.borrow(cs);
        *scheduler.get() = Some(Scheduler::new());
    }
}

/// Pushes a new task to be scheduled.
pub fn push<T: Context + Send + 'static>(task: T) {
    critical_section::with(|cs| {
        let scheduler = SCHEDULER.borrow(cs);
        unsafe { (*scheduler.get()).as_mut().unwrap().push(task) }
    })
}

/// Schedules a new task.
pub fn switch() -> ! {
    unsafe {
        let restore_state = critical_section::acquire();
        let cs = CriticalSection::new();
        let scheduler = SCHEDULER.borrow(cs);
        (*scheduler.get()).as_mut().unwrap().switch(restore_state);
    }
}
impl Scheduler {
    /// Creates an empty scheduler.
    pub fn new() -> Self {
        Self {
            readyq: VecDeque::new(),
            current: None,
        }
    }

    /// Pushes a new task to the scheduler.
    pub fn push<T: Context + Send + 'static>(&mut self, task: T) {
        let task = Box::new(task);
        self.readyq.push_back(task);
    }

    /// Schedules the next task to run.
    ///
    /// Note how this doesn't return. In order to reschedule to the next element,
    /// it's necessary to call this method again.
    ///
    /// The function will restore the critical section state before perfoming the context switch.
    pub unsafe fn switch(&mut self, restore_state: RestoreState) -> ! {
        loop {
            if let Some(task) = self.readyq.pop_front() {
                if task.completed() {
                    continue;
                }
                if let Some(previous) = self.current.replace(task) {
                    if !previous.completed() {
                        self.readyq.push_back(previous);
                    }
                }
                unsafe {
                    critical_section::release(restore_state);
                }
                self.current.as_ref().unwrap().switch();
            }
            panic!("No tasks to run");
        }
    }
}

impl Debug for Scheduler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if f.alternate() {
            write!(f, "Scheduler {{\n\treadyq: [\n")?;
            for task in self.readyq.iter() {
                write!(f, "\t\t{:?}\n", &*task as *const _)?;
            }
            write!(
                f,
                "],\n\tcurrent: {:?}\n}}",
                self.current.as_ref().map(|b| &*b as *const _)
            )?;
        } else {
            write!(f, "Scheduler {{ readyq: [")?;
            for task in self.readyq.iter() {
                write!(f, "{:?},", &*task as *const _)?;
            }
            write!(
                f,
                "], current: {:?} }}",
                self.current.as_ref().map(|b| &*b as *const _)
            )?;
        }
        Ok(())
    }
}
