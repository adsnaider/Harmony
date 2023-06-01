//! The kernel scheduler.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::fmt::Debug;

use critical_section::Mutex;
use once_cell::sync::OnceCell;

use crate::arch::context::Context;

/// The kernel scheduler.
#[derive(Debug)]
pub struct Scheduler {
    readyq: Mutex<UnsafeCell<VecDeque<Context>>>,
    _blocked: Mutex<UnsafeCell<Vec<Context>>>,
    current: Mutex<UnsafeCell<Context>>,
}

static SCHEDULER: OnceCell<Scheduler> = OnceCell::new();

/// Initializes the scheduler.
pub fn init() {
    SCHEDULER.set(Scheduler::new(Context::main())).unwrap();
}

/// Pushes a new task to be scheduled.
pub fn push(task: Context) {
    SCHEDULER.get().unwrap().push(task);
}

/// Performs a context switch.
pub fn switch() {
    SCHEDULER.get().unwrap().switch();
}

/// Terminates the currently running task and schedules the next one
pub fn exit() -> ! {
    SCHEDULER.get().unwrap().exit();
}

impl Scheduler {
    /// Creates an empty scheduler.
    pub fn new(current: Context) -> Self {
        Self {
            readyq: Mutex::new(UnsafeCell::new(VecDeque::new())),
            current: Mutex::new(UnsafeCell::new(current)),
            _blocked: Mutex::new(UnsafeCell::new(Vec::new())),
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
        critical_section::with(|cs| {
            let readyq = unsafe { &mut *self.readyq.borrow(cs).get() };
            let current = unsafe { &mut *self.current.borrow(cs).get() };
            if let Some(next) = readyq.pop_front() {
                let previous = core::mem::replace(current, next);
                readyq.push_back(previous);
                let previous = readyq.back_mut().unwrap();
                let next = current;
                // SAFETY: This is super awkward but hopefully safe.
                // * It's probably not cool to keep references that need to live after the switch, so we use raw pointers.
                // * The pointers only become references before the switch, not after.
                // * When the switch comes back to us (on a further restore, we don't have any more references around).
                unsafe {
                    // This function will restore interrupts
                    Context::switch(next, previous);
                }
            }
        })
    }

    /// Terminates the currently running task and schedules the next one
    pub fn exit(&self) -> ! {
        loop {
            critical_section::with(|cs| {
                let readyq = unsafe { &mut *self.readyq.borrow(cs).get() };
                let current = unsafe { &mut *self.current.borrow(cs).get() };
                if let Some(next) = readyq.pop_front() {
                    *current = next;
                    let next = current;
                    unsafe {
                        Context::jump(next);
                    }
                }
            });
            assert!(crate::arch::interrupts::are_enabled());
            crate::arch::inst::hlt();
        }
    }
}
