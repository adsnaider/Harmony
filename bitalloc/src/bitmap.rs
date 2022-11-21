//! An implementaiton of boolean bitmap.

#![allow(dead_code)]

mod unsigned_numeric;
use unsigned_numeric::UnsignedNumeric;

/// The booloean bitmap.
#[derive(Debug, Eq, PartialEq)]
pub struct Bitmap<'a, U: UnsignedNumeric> {
    /// The buffer.
    buffer: &'a mut [U],
}

impl<'a, U: UnsignedNumeric> Bitmap<'a, U> {
    /// Constructs a new bitmap with all values being zero (false).
    pub fn zeros(buffer: &'a mut [U]) -> Self {
        buffer.fill(U::ZERO);
        Self { buffer }
    }

    /// Get the value at index `i`.
    pub fn get(&self, i: usize) -> bool {
        let byte_addr = i / U::BITS;
        let bit_offset = i % U::BITS;
        ((self.buffer[byte_addr] >> bit_offset) & U::ONE) == U::ONE
    }

    /// Sets the value at index `i` to `true`.
    pub fn set(&mut self, i: usize) {
        let byte_addr = i / U::BITS;
        let bit_offset = i % U::BITS;
        self.buffer[byte_addr] |= U::ONE << bit_offset;
    }

    /// Sets the value at index `i` to `false`.
    pub fn unset(&mut self, i: usize) {
        let byte_addr = i / U::BITS;
        let bit_offset = i % U::BITS;
        self.buffer[byte_addr] &= !(U::ONE << bit_offset);
    }

    /// Returns the index of the first element set or returns None if all elements are unset.
    pub fn find_first_set(&mut self) -> Option<usize> {
        let (idx, elem) = self
            .buffer
            .iter()
            .enumerate()
            .find(|&(_, &x)| x != U::ZERO)?;
        let offset = elem.trailing_zeros() as usize;
        Some(idx * U::BITS + offset)
    }

    /// Returns the index of the first element unset or returns None if all elements are set.
    pub fn find_first_unset(&mut self) -> Option<usize> {
        let (idx, elem) = self
            .buffer
            .iter()
            .enumerate()
            .find(|&(_, &x)| x != U::MAX)?;
        let offset = elem.trailing_ones() as usize;
        Some(idx * U::BITS + offset)
    }
    /// Truncates the bitmap and returns the leftover storage.
    pub fn truncate(self, count: usize) -> (Self, &'a mut [U]) {
        let (truncated, leftover) = self.buffer.split_at_mut((count - 1) / U::BITS + 1);
        (Self { buffer: truncated }, leftover)
    }

    /// Returns the capacity of the bitmap.
    pub fn len(&self) -> usize {
        self.buffer.len() * U::BITS
    }
}

#[cfg(test)]
pub mod tests {
    use super::Bitmap;

    #[test]
    fn simple() {
        let mut storage = vec![0u32; 1024];
        let mut bitmap = Bitmap::zeros(&mut storage);

        bitmap.set(3);
        assert!(!bitmap.get(2));
        assert!(bitmap.get(3));
        assert!(!bitmap.get(4));
        assert!(!bitmap.get(35));
        assert_eq!(bitmap.find_first_set().unwrap(), 3);
        assert_eq!(bitmap.find_first_unset().unwrap(), 0);

        let (mut bitmap, leftover) = bitmap.truncate(4);
        assert_eq!(leftover.len(), 1023);
        assert_eq!(bitmap.len(), 32);
        assert!(bitmap.get(3));
        bitmap.unset(3);
        assert!(!bitmap.get(3));
    }
}
