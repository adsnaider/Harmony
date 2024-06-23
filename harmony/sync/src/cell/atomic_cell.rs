//! A lock-free implementaiton of a Send/Sync Cell.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub struct AtomicCell<T> {
    value: UnsafeCell<T>,
    locked: AtomicBool,
}

impl<T: Default> Default for AtomicCell<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> AtomicCell<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    pub fn set(&self, value: T) {
        self.spin_lock(|inner| {
            *inner = value;
        });
    }

    pub fn replace(&self, value: T) -> T {
        self.spin_lock(|inner| core::mem::replace(inner, value))
    }

    #[inline(always)]
    fn spin_lock<U, F: FnOnce(&mut T) -> U>(&self, fun: F) -> U {
        self.lock();
        let out = fun(unsafe { &mut *self.value.get() });
        self.unlock();
        out
    }

    #[inline(always)]
    fn lock(&self) {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {}
    }
    #[inline(always)]
    fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

impl<T: Copy> AtomicCell<T> {
    pub fn get(&self) -> T {
        self.spin_lock(|inner| *inner)
    }
}

impl<T: Clone> AtomicCell<T> {
    pub fn get_cloned(&self) -> T {
        self.spin_lock(|inner| inner.clone())
    }
}
