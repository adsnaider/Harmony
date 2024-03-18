use core::cell::Cell;
use core::ops::{Deref, DerefMut};

use super::AtomicOnceCell;

pub struct AtomicLazyCell<T, F = fn() -> T> {
    inner: AtomicOnceCell<T>,
    fun: Cell<Option<F>>,
}

// SAFETY: We never create a `&F` from a `&Lazy<T, F>` so it is fine to not impl
// `Sync` for `F`. We do create a `&mut Option<F>` in `force`, but this is
// properly synchronized, so it only happens once so it also does not
// contribute to this impl.
unsafe impl<T, F: Send> Sync for AtomicLazyCell<T, F> where AtomicOnceCell<T>: Sync {}

impl<T, F: FnOnce() -> T> AtomicLazyCell<T, F> {
    pub const fn new(fun: F) -> Self {
        Self {
            inner: AtomicOnceCell::new(),
            fun: Cell::new(Some(fun)),
        }
    }

    pub fn get(&self) -> &T {
        let _ = self.inner.set_with(|| match self.fun.take() {
            Some(fun) => fun(),
            None => panic!("Lazy instance has previously been poisoned"),
        });
        loop {
            match self.inner.get() {
                Some(value) => break value,
                // Still initializing
                None => {}
            }
        }
    }

    pub fn get_mut(&mut self) -> &mut T {
        let _ = self.inner.set_with(|| match self.fun.take() {
            Some(fun) => fun(),
            None => panic!("Lazy instance has previously been poisoned"),
        });
        match self.inner.get_mut() {
            Some(value) => value,
            // Still initializing
            None => panic!("Lazy instance should have been initialized"),
        }
    }
}

impl<T, F: Fn() -> T> Deref for AtomicLazyCell<T, F> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T, F: Fn() -> T> DerefMut for AtomicLazyCell<T, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

#[cfg(test)]
mod tests {
    use core::hint::black_box;
    use core::time::Duration;

    use super::*;

    #[test]
    fn multithreaded_evaluation() {
        let cell = AtomicLazyCell::new(|| {
            // Make a long running computation
            std::thread::sleep(Duration::from_secs(1));
            // Black box it just in case.
            black_box(10)
        });

        std::thread::scope(|s| {
            for _ in 0..10 {
                s.spawn(|| {
                    let value = cell.get();
                    assert_eq!(*value, 10);
                });
            }
        });
    }
}
