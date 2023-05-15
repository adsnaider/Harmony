//! The kernel scheduler.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::fmt::Debug;

use critical_section::{CriticalSection, Mutex};
use once_cell::sync::OnceCell;

use crate::arch::context::Context;

/// The kernel scheduler.
#[derive(Debug)]
pub struct Scheduler {
    readyq: Mutex<UnsafeCell<VecDeque<Context>>>,
    blocked: Mutex<UnsafeCell<Vec<Context>>>,
    current: Mutex<UnsafeCell<Option<Context>>>,
}

static SCHEDULER: OnceCell<Scheduler> = OnceCell::new();

/// Initializes the scheduler.
pub fn init() {
    SCHEDULER.set(Scheduler::new()).unwrap();
}

/// Pushes a new task to be scheduled.
pub fn push(task: Context) {
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
pub fn terminate() -> ! {
    SCHEDULER.get().unwrap().terminate();
}

impl Scheduler {
    /// Creates an empty scheduler.
    pub fn new() -> Self {
        Self {
            readyq: Mutex::new(UnsafeCell::new(VecDeque::new())),
            current: Mutex::new(UnsafeCell::new(None)),
            blocked: Mutex::new(UnsafeCell::new(Vec::new())),
        }
    }

    /// Pushes a new task to the scheduler.
    pub fn push(&self, task: Context) {
        // SAFETY: This operation is non-reentrant.
        critical_section::with(|cs| unsafe {
            (*self.readyq.borrow(cs).get()).push_back(task);
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
        let readyq = unsafe { &mut *self.readyq.borrow(cs).get() };
        let current = unsafe { &mut *self.current.borrow(cs).get() };
        if let Some(next) = readyq.pop_front() {
            let previous = current.replace(next).unwrap();
            readyq.push_back(previous);
            let previous: *mut Context = readyq.back_mut().unwrap();
            let next: *const Context = current.as_ref().unwrap();
            drop(readyq);
            drop(current);
            // SAFETY: This is super awkward but hopefully safe.
            // * It's probably not cool to keep references that need to live after the switch, so we use raw pointers.
            // * The pointers only become references before the switch, not after.
            // * By the time we reenable interrupts and jump to the next context, we don't use the pointers anymore.
            // * When the switch comes back to us (on a further restore, we don't have any more references around).
            unsafe {
                // This function will restore interrupts
                Context::switch(next, previous);
            }
        }
    }

    /// Starts the scheduler.
    ///
    /// This function can only be called once to initialize the scheduling model.
    /// Doing so will cause a crash.
    pub fn run(&self) -> ! {
        assert!(crate::arch::int::are_enabled());
        crate::arch::int::disable();
        let cs = unsafe { CriticalSection::new() };
        let readyq = unsafe { &mut *self.readyq.borrow(cs).get() };
        let current = unsafe { &mut *self.current.borrow(cs).get() };
        let next = readyq.pop_front().unwrap();
        assert!(current.replace(next).is_none());
        let next: *const Context = current.as_ref().unwrap();
        drop(readyq);
        drop(current);

        // SAFETY: This is super awkward but hopefully safe.
        // * It's probably not cool to keep references that need to live after the switch, so we use raw pointers.
        // * The pointers only become references before the switch, not after.
        // * By the time we reenable interrupts and jump to the next context, we don't use the pointers anymore.
        // * When the switch comes back to us (on a further restore, we don't have any more references around).
        unsafe {
            Context::jump(next);
        }
    }

    /// Terminates the currently running task and schedules the next one
    pub fn terminate(&self) -> ! {
        assert!(crate::arch::int::are_enabled());
        loop {
            crate::arch::int::disable();
            let cs = unsafe { CriticalSection::new() };
            let readyq = unsafe { &mut *self.readyq.borrow(cs).get() };
            let current = unsafe { &mut *self.current.borrow(cs).get() };
            if let Some(next) = readyq.pop_front() {
                let old_ctx = current
                    .replace(next)
                    .expect("Called terminate before scheduler was running");
                let next: *const Context = current.as_ref().unwrap();
                drop(old_ctx);
                drop(readyq);
                drop(current);
                unsafe {
                    Context::jump(next);
                }
            } else {
                crate::arch::int::enable();
                crate::arch::inst::hlt();
            }
        }
    }
}
