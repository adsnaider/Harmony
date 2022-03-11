//! Live statics in the kernel.

use core::cell::{Ref, RefCell, RefMut};

// TODO(#14): Once async (or interrupts) we should use an async lock instead of a RefCell.
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
