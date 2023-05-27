//! Wrapper type for system singletons.

#![no_std]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks)]

use core::cell::{RefCell, RefMut};
use core::ops::DerefMut;

use critical_section::{CriticalSection, Mutex};

/// A spinlock-protected value.
#[derive(Debug)]
pub struct Singleton<T: Send> {
    value: Mutex<RefCell<Option<T>>>,
}

impl<T: Send> Singleton<T> {
    /// Constructs an uninitialized singleton.
    pub const fn uninit() -> Self {
        Self {
            value: Mutex::new(RefCell::new(None)),
        }
    }

    /// Initializes the singleton with the `value`.
    pub fn initialize(&self, value: T, cs: CriticalSection) {
        let mut inner = self.value.borrow_ref_mut(cs);
        assert!(inner.is_none(), "Can't initialize singleton twice!");
        *inner = Some(value);
    }

    /// Mutably locks the singleton.
    pub fn lock<'a>(&'a self, cs: CriticalSection<'a>) -> impl DerefMut<Target = T> + 'a {
        RefMut::map(self.value.borrow_ref_mut(cs), |t| t.as_mut().unwrap())
    }

    /// Returns true if the singleton has been initialized.
    pub fn is_init(&self, cs: CriticalSection) -> bool {
        self.value.borrow_ref(cs).is_some()
    }

    /// Sets the value in the singleton to `value`.
    ///
    /// Essentially the same as `initialize` with the difference that
    /// it won't panic if a value is already present.
    pub fn set(&self, value: T, cs: CriticalSection) {
        let mut inner = self.value.borrow_ref_mut(cs);
        *inner = Some(value);
    }

    /// Perform an operation on the locked value.
    pub fn locked<O>(&self, cs: CriticalSection, actor: impl FnOnce(&mut T) -> O) -> O {
        let mut data = self.lock(cs);
        actor(&mut *data)
    }

    /// Locks the underlying data without the need of a critical section.
    pub unsafe fn lock_unchecked<'a>(&'a self) -> impl DerefMut<Target = T> + 'a {
        unsafe {
            let cs = CriticalSection::new();
            self.lock(cs)
        }
    }
}
