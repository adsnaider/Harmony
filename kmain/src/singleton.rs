//! Wrapper type for system singletons.

use core::ops::{Deref, DerefMut};

use spin::Mutex;

/// A spinlock-protected value.
pub struct Singleton<T: Send> {
    value: Mutex<Option<T>>,
}

struct Guard<'a, T>(spin::MutexGuard<'a, Option<T>>);

impl<'a, T> Deref for Guard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("Uninitialized singleton.")
    }
}

impl<'a, T> DerefMut for Guard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("Uninitialized singleton.")
    }
}

impl<T: Send> Singleton<T> {
    /// Constructs an uninitialized singleton.
    pub const fn uninit() -> Self {
        Self {
            value: Mutex::new(None),
        }
    }

    /// Initializes the singleton with the `value`.
    pub fn initialize(&self, value: T) {
        let mut inner = self.value.lock();
        assert!(inner.is_none(), "Can't initialize singleton twice!");
        *inner = Some(value);
    }

    /// Mutably locks the singleton.
    pub fn lock(&self) -> impl DerefMut<Target = T> + '_ {
        Guard(self.value.lock())
    }

    /// Tries to lock the singleton.
    ///
    /// # Returns
    ///
    /// * Some(...) if the mutex lock was acquired.
    /// * None if the mutex is currently locked.
    pub fn try_lock(&self) -> Option<impl DerefMut<Target = T> + '_> {
        self.value.try_lock().map(|guard| Guard(guard))
    }
}
