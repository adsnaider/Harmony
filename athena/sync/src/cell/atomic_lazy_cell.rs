use core::cell::Cell;
use core::ops::Deref;

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
        unsafe { self.inner.get_unchecked() }
    }
}

impl<T, F: Fn() -> T> Deref for AtomicLazyCell<T, F> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}
