//! Wrapper type for system singletons.

#![allow(dead_code)]

use core::ops::DerefMut;

use spin::Mutex;

/// A spinlock-protected value.
#[derive(Debug)]
pub struct Singleton<T: Send> {
    value: Mutex<Option<T>>,
}

pub(super) mod guards {
    use core::ops::{Deref, DerefMut};
    pub struct Guard<'a, T>(spin::MutexGuard<'a, Option<T>>);

    impl<'a, T> Guard<'a, T> {
        pub fn new(value: spin::MutexGuard<'a, Option<T>>) -> Self {
            Self(value)
        }
    }

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

    pub struct UnsafeGuard<'a, T>(spin::MutexGuard<'a, Option<T>>);

    impl<'a, T> UnsafeGuard<'a, T> {
        /// Construct an `UnsafeGuard`.
        ///
        /// # Safety
        ///
        /// The provided mutex must always be `Some(...)` while the `UnsafeGuard` exists.
        pub unsafe fn new(value: spin::MutexGuard<'a, Option<T>>) -> Self {
            Self(value)
        }
    }

    impl<'a, T> Deref for UnsafeGuard<'a, T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            // SAFETY: See the `new` method.
            unsafe { self.0.as_ref().unwrap_unchecked() }
        }
    }

    impl<'a, T> DerefMut for UnsafeGuard<'a, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            // SAFETY: See the `new` method.
            unsafe { self.0.as_mut().unwrap_unchecked() }
        }
    }
}

use guards::{Guard, UnsafeGuard};

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
        Guard::new(self.value.lock())
    }

    /// Mutably locks the singleton as an `Option`
    ///
    /// This will spin until the lock has been acquired.
    pub fn lock_option(&self) -> Option<impl DerefMut<Target = T> + '_> {
        if self.is_init() {
            // SAFETY: We checked that the singleton has been initialized. Once initialized,
            // there's no way for outside code to uninitialize it since the dereferenced value
            // provided by the lock is of type T and not Option<T>
            Some(unsafe { UnsafeGuard::new(self.value.lock()) })
        } else {
            None
        }
    }

    /// Tries to lock the singleton.
    ///
    /// # Returns
    ///
    /// * Some(...) if the mutex lock was acquired.
    /// * None if the mutex is currently locked.
    pub fn try_lock(&self) -> Option<impl DerefMut<Target = T> + '_> {
        self.value.try_lock().map(|guard| Guard::new(guard))
    }

    /// Returns true if the singleton has been initialized.
    pub fn is_init(&self) -> bool {
        self.value.lock().is_some()
    }
}
