use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{fence, AtomicU8, Ordering};

pub struct AtomicOnceCell<T> {
    value: UnsafeCell<MaybeUninit<T>>,
    init: AtomicU8,
}

pub enum OnceError {
    Initializing,
    AlreadyInit,
}

impl<T> AtomicOnceCell<T> {
    pub fn new() -> Self {
        Self {
            value: UnsafeCell::new(MaybeUninit::uninit()),
            init: AtomicU8::new(0),
        }
    }

    pub fn set(&self, value: T) -> Result<(), OnceError> {
        match self
            .init
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Relaxed)
        {
            // SAFETY: Okay to write while initializing because we only allow 1 reference to exist
            Ok(_) => unsafe {
                (*self.value.get()).write(value);
            },
            Err(1) => return Err(OnceError::Initializing),
            Err(2) => return Err(OnceError::AlreadyInit),
            Err(other) => panic!("Unknown initialization state: {other}"),
        }
        self.init.store(2, Ordering::Release);
        Ok(())
    }

    pub fn get(&self) -> Option<&T> {
        let is_init = self.init.load(Ordering::Acquire) == 2;
        if is_init {
            // SAFETY: The value has been initialized and from now on, we only provide
            // shared references
            unsafe { Some(self.get_unchecked()) }
        } else {
            None
        }
    }

    /// Returns a shared reference to the inner value and assumes its been initialized.
    ///
    /// # Safety
    ///
    /// The inner value must be initialized at this point.
    pub unsafe fn get_unchecked(&self) -> &T {
        fence(Ordering::Acquire);
        // SAFETY: The value is assumed to have been initialized and from then on,
        // we only provide  shared references
        unsafe { (*self.value.get()).assume_init_ref() }
    }
}
