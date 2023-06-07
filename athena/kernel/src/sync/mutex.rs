//! The mighty mutex provides mutual exclusion to a single resource.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};

use super::sem::Semaphore;

/// Mutual exclusion primitive to provide exclusive access to data.
#[derive(Debug)]
pub struct Mutex<T> {
    data: UnsafeCell<T>,
    sem: Semaphore,
}

// SAFETY: Mutex is sole owner of the data.
unsafe impl<T: Send> Send for Mutex<T> {}
// SAFETY: Only one reference can be acquired at a time.
// `Send` bound is necessary in case the `T` gets replaced on the thread.
unsafe impl<T: Send> Sync for Mutex<T> {}

/// An RAII-like Mutex guard that provides access to the data.
#[derive(Debug)]
pub struct MutexGuard<'a, T> {
    lock: &'a Mutex<T>,
}

// SAFETY: This is actually more restrictive.
// Needed to avoid sharing references across threads that don't implement Sync.
unsafe impl<T: Sync> Sync for MutexGuard<'_, T> {}
impl<T> !Send for MutexGuard<'_, T> {}

impl<T> Mutex<T> {
    /// Creates a new mutex in the unlocked state.
    pub fn new(t: T) -> Self {
        Self {
            data: UnsafeCell::new(t),
            sem: Semaphore::new(1),
        }
    }

    /// Acquires the mutex, potentially locking the thread until it's able to do so.
    pub fn lock(&self) -> MutexGuard<'_, T> {
        self.sem.wait();
        MutexGuard { lock: self }
    }

    /// Get a mutable reference to the underlying data.
    ///
    /// Since this method takes `&mut self`, no locks are actually acquired.
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// Consumes the mutex, returning the underlying data.
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: We have the mutex guard.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: We have the mutex guard.
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        // No more references exist, safe to signal.
        self.lock.sem.signal();
    }
}
