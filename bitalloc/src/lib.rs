//! A frame allocator that manages frame with a bitmap.
#![cfg_attr(not(test), no_std)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]

mod bitmap;

use core::borrow::Borrow;
use core::marker::PhantomData;

use self::bitmap::Bitmap;

/// An `Indexable` type is one that can be constructed from a 0-based index.
///
/// # Safety
///
/// For correctness, this property should be symmetric (an index should be mapped to 1 and only 1
/// element).
pub unsafe trait Indexable {
    /// Returns the index corresponding to `self`.
    fn index(&self) -> usize;

    /// Returns the element corresponding to `idx`.
    fn from_index(idx: usize) -> Self;
}

/// An object allocator that uses a bitmap to keep track of available frames.
#[derive(Debug)]
pub struct Bitalloc<'a, T: Indexable> {
    /// We store availability through the bitmap.
    store: Bitmap<'a, u64>,
    /// Number of elements.
    count: usize,
    /// Phantom...
    _phantom: PhantomData<T>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
/// Bit allocation error.
pub enum BitAllocError {
    /// No more elements available to allocate.
    OutOfElements,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
/// Bit deallocation error.
pub enum BitDeallocError {
    /// Element wasn't allocated.
    NotAllocated,
    /// Attempted to allocate an element whose index exceeds the capacity.
    OutOfRange,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
/// Bit allocate specific error.
pub enum BitSpecificError {
    /// Attempted to allocate an element that is unavailable.
    Unavailable,
    /// Attempted to allocate an element whose index exceeds the capacity.
    OutOfRange,
}

impl<'a, T: Indexable> Bitalloc<'a, T> {
    /// Builds the bitmap allocator and returns any leftover storage.
    pub fn new(store: &'a mut [u64], count: usize) -> (Self, &'a mut [u64]) {
        if count > store.len() * 64 {
            panic!("Storage provided not large enough.");
        }
        let (bitmap, leftover) = Bitmap::zeros(store).truncate(count);

        (
            Self {
                store: bitmap,
                count,
                _phantom: PhantomData,
            },
            leftover,
        )
    }

    /// Constructs a new bitmap allocator with the specified availability. It also returns unneeded
    /// storage.
    pub fn new_with_availability<'b, I, Q>(
        store: &'a mut [u64],
        count: usize,
        unavailable: I,
    ) -> (Self, &'a mut [u64])
    where
        Q: Borrow<T>,
        I: IntoIterator<Item = Q>,
    {
        let (mut this, leftover) = Self::new(store, count);
        for unavail in unavailable.into_iter() {
            this.store.set(unavail.borrow().index());
        }
        (this, leftover)
    }

    /// Allocates some element and returns it.
    pub fn allocate(&mut self) -> Result<T, BitAllocError> {
        let avail = self
            .store
            .find_first_unset()
            .ok_or(BitAllocError::OutOfElements)?;
        if avail >= self.count {
            Err(BitAllocError::OutOfElements)
        } else {
            self.store.set(avail);
            Ok(T::from_index(avail))
        }
    }

    /// Deallocates the specified element.
    pub fn deallocate(&mut self, t: &T) -> Result<(), BitDeallocError> {
        let idx = t.index();
        if idx >= self.count {
            return Err(BitDeallocError::OutOfRange);
        }
        if self.store.get(idx) {
            self.store.unset(idx);
            Ok(())
        } else {
            Err(BitDeallocError::NotAllocated)
        }
    }

    /// Attempts to allocate a specific element.
    pub fn allocate_specific(&mut self, t: &T) -> Result<(), BitSpecificError> {
        let idx = t.index();
        if idx >= self.count {
            return Err(BitSpecificError::OutOfRange);
        }
        if !self.store.get(idx) {
            self.store.set(idx);
            Ok(())
        } else {
            Err(BitSpecificError::Unavailable)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
    struct Something(usize);

    // SAFETY: Each element is uniquely and symmetrically mapped.
    unsafe impl Indexable for Something {
        fn index(&self) -> usize {
            self.0 / 32
        }

        fn from_index(idx: usize) -> Self {
            Self(idx * 32)
        }
    }

    #[test]
    fn simple() {
        let mut storage = vec![0u64; 1024];
        let mut current_allocations = HashSet::new();
        let (mut bitalloc, leftover) = Bitalloc::new(&mut storage, 2500);
        // 2500 / 64 = 39.xx
        assert_eq!(leftover.len(), 1024 - 40);

        for _ in 0..1000 {
            let allocation: Something = bitalloc
                .allocate()
                .expect("Should have plenty of capacity left.");
            assert!(current_allocations.insert(allocation), "Double allocation!");
        }

        for allocation in current_allocations.iter() {
            bitalloc
                .deallocate(allocation)
                .expect("All elements should have been previously allocated.");
        }
        current_allocations.clear();

        for _ in 0..2500 {
            let allocation: Something = bitalloc
                .allocate()
                .expect("Should have plenty of capacity left.");
            assert!(current_allocations.insert(allocation), "Double allocation!");
        }
        bitalloc.allocate().expect_err("No more left to allocate.");

        for _ in 0..50 {
            let to_dealloc: Vec<Something> = current_allocations.iter().copied().take(10).collect();
            for to_dealloc in to_dealloc.iter() {
                bitalloc.deallocate(&to_dealloc).unwrap();
                current_allocations.remove(&to_dealloc);
            }

            for _ in 0..9 {
                let allocation = bitalloc.allocate().unwrap();
                assert!(current_allocations.insert(allocation), "Double allocation!");
            }
        }
    }

    #[test]
    fn test_specific_allocations() {
        let mut storage = vec![0u64; 1];
        let (mut bitalloc, _) = Bitalloc::new(&mut storage, 64);

        bitalloc
            .allocate_specific(&Something::from_index(26))
            .unwrap();

        bitalloc
            .allocate_specific(&Something::from_index(14))
            .unwrap();

        bitalloc
            .allocate_specific(&Something::from_index(10))
            .unwrap();

        bitalloc
            .allocate_specific(&Something::from_index(11))
            .unwrap();

        bitalloc
            .allocate_specific(&Something::from_index(13))
            .unwrap();

        assert_eq!(
            bitalloc.allocate_specific(&Something::from_index(14)),
            Err(BitSpecificError::Unavailable),
        );

        bitalloc
            .allocate_specific(&Something::from_index(12))
            .unwrap();

        assert_eq!(
            bitalloc.allocate_specific(&Something::from_index(26)),
            Err(BitSpecificError::Unavailable),
        );

        bitalloc.deallocate(&Something::from_index(11)).unwrap();
        bitalloc
            .allocate_specific(&Something::from_index(11))
            .unwrap();
    }

    #[test]
    fn test_with_start_conditions() {
        let mut storage = vec![0u64; 1];
        let (mut bitalloc, _) = Bitalloc::new_with_availability(
            &mut storage,
            64,
            vec![
                Something::from_index(1),
                Something::from_index(2),
                Something::from_index(3),
                Something::from_index(27),
            ],
        );

        assert_eq!(
            bitalloc.allocate_specific(&Something::from_index(1)),
            Err(BitSpecificError::Unavailable)
        );

        assert_eq!(
            bitalloc.allocate_specific(&Something::from_index(2)),
            Err(BitSpecificError::Unavailable)
        );
        assert_eq!(
            bitalloc.allocate_specific(&Something::from_index(3)),
            Err(BitSpecificError::Unavailable)
        );
        assert_eq!(
            bitalloc.allocate_specific(&Something::from_index(27)),
            Err(BitSpecificError::Unavailable)
        );

        bitalloc
            .allocate_specific(&Something::from_index(0))
            .unwrap();
        bitalloc
            .allocate_specific(&Something::from_index(4))
            .unwrap();

        // 64 elements - 4 unavailabe - 2 allocated = 58

        for _ in 0..58 {
            bitalloc.allocate().unwrap();
        }

        assert_eq!(bitalloc.allocate(), Err(BitAllocError::OutOfElements));
        assert_eq!(bitalloc.allocate(), Err(BitAllocError::OutOfElements));
        bitalloc.deallocate(&Something::from_index(54)).unwrap();
        bitalloc.allocate().unwrap();
    }
}
