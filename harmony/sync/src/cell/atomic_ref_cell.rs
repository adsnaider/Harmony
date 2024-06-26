//! A lock-free implementaiton of a Send/Sync RefCell.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU8, Ordering};

unsafe impl<T: Send> Send for AtomicRefCell<T> {}
// FIXME: Only doing exclusive borrows for now.
// TODO: Add Send bound on T when non-exclusive borrows
unsafe impl<T: Send> Sync for AtomicRefCell<T> {}

#[derive(Debug)]
pub struct AtomicRefCell<T> {
    data: UnsafeCell<T>,
    state: State,
}

#[derive(Debug)]
pub struct State(AtomicU8);

impl State {
    const FREE: u8 = 0;
    const BORROWED: u8 = 1;

    pub const fn free() -> Self {
        Self(AtomicU8::new(Self::FREE))
    }

    pub fn try_borrow(&self) -> Result<(), BorrowError> {
        match self.0.compare_exchange(
            Self::FREE,
            Self::BORROWED,
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(()),
            Err(_) => Err(BorrowError::AlreadyBorrowed),
        }
    }

    pub fn drop_borrow(&self) {
        assert_eq!(self.0.swap(Self::FREE, Ordering::AcqRel), Self::BORROWED);
    }
}

#[derive(Debug)]
pub enum BorrowError {
    AlreadyBorrowed,
}

impl<T: Default> Default for AtomicRefCell<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> AtomicRefCell<T> {
    pub const fn new(value: T) -> Self {
        Self {
            data: UnsafeCell::new(value),
            state: State::free(),
        }
    }

    pub fn borrow(&self) -> Result<Ref<T>, BorrowError> {
        self.state.try_borrow()?;
        unsafe {
            Ok(Ref {
                value: NonNull::new_unchecked(self.data.get()),
                borrow: BorrowRef(&self.state),
            })
        }
    }

    pub fn borrow_mut(&self) -> Result<RefMut<T>, BorrowError> {
        self.state.try_borrow()?;
        unsafe {
            Ok(RefMut {
                value: NonNull::new_unchecked(self.data.get()),
                borrow: BorrowRefMut(&self.state),
            })
        }
    }
}

struct BorrowRef<'a>(&'a State);
struct BorrowRefMut<'a>(&'a State);

pub struct Ref<'a, T> {
    value: NonNull<T>,
    borrow: BorrowRef<'a>,
}

impl<'a, T> Ref<'a, T> {
    pub fn map<F, U>(this: Self, fun: F) -> Ref<'a, U>
    where
        F: FnOnce(&T) -> &U,
    {
        Ref {
            value: fun(&*this).into(),
            borrow: this.borrow,
        }
    }
}

impl<T> Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.value.as_ptr() }
    }
}

pub struct RefMut<'a, T> {
    value: NonNull<T>,
    borrow: BorrowRefMut<'a>,
}

impl<'a, T> RefMut<'a, T> {
    pub fn map<F, U>(this: Self, fun: F) -> RefMut<'a, U>
    where
        F: FnOnce(&T) -> &U,
    {
        RefMut {
            value: fun(&*this).into(),
            borrow: this.borrow,
        }
    }
}

impl<T> Deref for RefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.value.as_ptr() }
    }
}

impl<T> DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.value.as_ptr() }
    }
}

impl Drop for BorrowRef<'_> {
    fn drop(&mut self) {
        self.0.drop_borrow();
    }
}

impl Drop for BorrowRefMut<'_> {
    fn drop(&mut self) {
        self.0.drop_borrow();
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn threaded_mutability() {
        let cell = AtomicRefCell::new(0);

        #[cfg(miri)]
        const ITERS: i32 = 100;
        #[cfg(not(miri))]
        const ITERS: i32 = 100000;

        std::thread::scope(|s| {
            s.spawn(|| {
                for i in 0..ITERS {
                    if let Ok(mut value) = cell.borrow_mut() {
                        *value = i;
                        std::thread::yield_now();
                        assert_eq!(*value, i);
                    }
                    std::thread::yield_now();
                }
            });
            s.spawn(|| {
                for i in ITERS..2 * ITERS {
                    if let Ok(mut value) = cell.borrow_mut() {
                        *value = i;
                        std::thread::yield_now();
                        assert_eq!(*value, i);
                    }
                    std::thread::yield_now();
                }
            });
            s.spawn(|| {
                for i in 2 * ITERS..3 * ITERS {
                    if let Ok(mut value) = cell.borrow_mut() {
                        *value = i;
                        std::thread::yield_now();
                        assert_eq!(*value, i);
                    }
                    std::thread::yield_now();
                }
            });
            s.spawn(|| {
                for i in 3 * ITERS..4 * ITERS {
                    if let Ok(mut value) = cell.borrow_mut() {
                        *value = i;
                        std::thread::yield_now();
                        assert_eq!(*value, i);
                    }
                    std::thread::yield_now();
                }
            });
        });
    }
}
