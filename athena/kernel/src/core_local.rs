use core::marker::PhantomData;
use core::mem::MaybeUninit;

// FIXME: Make this an actual core-local api.

const NUM_CORES: usize = 1;

#[repr(transparent)]
pub struct CoreLocal<T> {
    values: [T; NUM_CORES],
    _phantom: PhantomData<*mut ()>,
}

impl<T> CoreLocal<T> {
    pub fn new_with<F>(fun: F) -> Self
    where
        F: Fn(usize) -> T,
    {
        Self {
            values: core::array::from_fn(fun),
            _phantom: PhantomData,
        }
    }

    pub fn get(&self) -> &T {
        &self.values[0]
    }
}

impl<T: Copy> CoreLocal<T> {
    pub const fn new(value: T) -> Self {
        Self {
            values: [value; NUM_CORES],
            _phantom: PhantomData,
        }
    }
}

impl<T> CoreLocal<MaybeUninit<T>> {
    pub const fn new_uninit() -> Self {
        Self {
            // SAFETY: We can assume initialized a MaybeUninit array.
            values: unsafe { MaybeUninit::uninit().assume_init() },
            _phantom: PhantomData,
        }
    }
}

// SAFETY: non-preemption + each thread gets its own unit
unsafe impl<T> Sync for CoreLocal<T> {}
// SAFETY: Sending a Corelocal has no effect as the `get` method
// will always observe the local in the active thread
unsafe impl<T> Send for CoreLocal<T> {}
