//! Live statics in the kernel.

use core::cell::{Ref, RefCell, RefMut};

// The kernel runs in a single thread and can't be preempted
// since we are the kernel. That means all operations in the GlobalDisplay will finish without
// being stopped. Hoewver, we still can't borrow the display mutably while holding an immutable
// reference e.g:
// ```
// writeln(display, "{:?}", &display);
// ```
//
// We can keep this guarded in a RefCell to panic! in such case. However, since the panic
// implementation won't clean up memory (aborts) we may not be able to use the display there. For
// that reason, it's important that the panic implementation doesn't itself panic again (by trying
// to acquire the display).
//
// Exceptions:
//
// Eventually, we will add support for async programming to switch between I/O bound tasks.
// Then we can keep the display under an async lock.

// TODO(adsnaider): Once async, we should use an async lock instead of a RefCell.
/// A kernel `LiveStatic` is a structure that holds a value after being setup. The value will be
/// guarded by a RefCell.
#[derive(Debug)]
pub struct LiveStatic<T> {
    data: RefCell<Option<T>>,
}

/// Error returned when the resource can't be borrowed.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum StaticBorrowError {
    /// Couldn't borrow due to aliasing rules.
    ResourceInUse,
    /// Couldn't borrow because resource wasn't set.
    ResourceNotSet,
}

impl<T> LiveStatic<T> {
    /// Constructs a new LiveStatic that holds no data.
    pub const fn new() -> Self {
        Self {
            data: RefCell::new(None),
        }
    }

    /// Sets the value in `self` to `to`.
    ///
    /// After calling, `is_set()` should return true.
    pub fn set(&self, to: T) {
        *self.data.borrow_mut() = Some(to);
    }

    /// Resets the value of `self`, leaving None.
    pub fn reset(&self) {
        *self.data.borrow_mut() = None;
    }

    /// Returns true if the `self` has it's data set.
    pub fn is_set(&self) -> bool {
        self.data.borrow().is_some()
    }

    /// Borrows the data under `self`. May panic! if the data isn't set or if aliasing rules would
    /// be broken.
    pub fn borrow(&self) -> Ref<'_, T> {
        Ref::map(self.data.borrow(), |opt| opt.as_ref().unwrap())
    }

    /// Mutably borrows the data under `self`. May panic! if the data isn't set or if aliasing rules
    /// would be broken.
    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        RefMut::map(self.data.borrow_mut(), |opt| opt.as_mut().unwrap())
    }

    /// Borrows the data under `self`, returning an error if the data isn't set or if aliasing rules
    /// would be broken.
    pub fn try_borrow(&self) -> Result<Ref<'_, T>, StaticBorrowError> {
        let reff = self
            .data
            .try_borrow()
            .map_err(|_| StaticBorrowError::ResourceInUse)?;
        match *reff {
            Some(_) => Ok(Ref::map(reff, |opt| unsafe {
                opt.as_ref().unwrap_unchecked()
            })),
            None => Err(StaticBorrowError::ResourceNotSet),
        }
    }

    /// Mutably borrows the data under `self`, returning an error if the data isn't set or if
    /// aliasing rules would be broken.
    pub fn try_borrow_mut(&self) -> Result<RefMut<'_, T>, StaticBorrowError> {
        let reff = self
            .data
            .try_borrow_mut()
            .map_err(|_| StaticBorrowError::ResourceInUse)?;
        match *reff {
            Some(_) => Ok(RefMut::map(reff, |opt| unsafe {
                opt.as_mut().unwrap_unchecked()
            })),
            None => Err(StaticBorrowError::ResourceNotSet),
        }
    }
}

// SAFETY: Kernel runs in a single thread.
unsafe impl<T> Sync for LiveStatic<T> {}
