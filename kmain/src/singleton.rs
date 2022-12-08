//! Wrapper type for system singletons.

#![allow(dead_code)]

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
}
