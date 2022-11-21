//! Common trait for numerics that can be used with the bitmap.
use core::ops::{
    Add, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Shl, Shr, Sub,
};

/// Necessary numeric operations for the bitmap.
pub trait UnsignedNumeric:
    Copy
    + Clone
    + Eq
    + PartialEq
    + Shr<usize, Output = Self>
    + Shl<usize, Output = Self>
    + Add<Self>
    + Sub<Self>
    + BitAnd<Output = Self>
    + BitOr<Output = Self>
    + BitXor<Output = Self>
    + BitAndAssign
    + BitOrAssign
    + BitXorAssign
    + Not<Output = Self>
{
    /// Number of bits in the unsigned.
    const BITS: usize;
    /// The zero value.
    const ZERO: Self;
    /// The one value.
    const ONE: Self;
    /// The max value (all set)
    const MAX: Self;

    /// Number of trailing zeros in binary representation.
    fn trailing_zeros(&self) -> usize;

    /// Number of trailing zeros in binary representation.
    fn trailing_ones(&self) -> usize;
}
impl UnsignedNumeric for u8 {
    const BITS: usize = 8;
    const ZERO: Self = 0u8;
    const ONE: Self = 1u8;
    const MAX: Self = u8::MAX;

    fn trailing_zeros(&self) -> usize {
        (*self as u8).trailing_zeros() as usize
    }

    fn trailing_ones(&self) -> usize {
        (*self as u8).trailing_ones() as usize
    }
}
impl UnsignedNumeric for u16 {
    const BITS: usize = 16;
    const ZERO: Self = 0u16;
    const ONE: Self = 1u16;
    const MAX: Self = u16::MAX;

    fn trailing_zeros(&self) -> usize {
        (*self as u16).trailing_zeros() as usize
    }

    fn trailing_ones(&self) -> usize {
        (*self as u16).trailing_ones() as usize
    }
}
impl UnsignedNumeric for u32 {
    const BITS: usize = 32;
    const ZERO: Self = 0u32;
    const ONE: Self = 1u32;
    const MAX: Self = u32::MAX;

    fn trailing_zeros(&self) -> usize {
        (*self as u32).trailing_zeros() as usize
    }

    fn trailing_ones(&self) -> usize {
        (*self as u32).trailing_ones() as usize
    }
}
impl UnsignedNumeric for u64 {
    const BITS: usize = 64;
    const ZERO: Self = 0u64;
    const ONE: Self = 1u64;
    const MAX: Self = u64::MAX;

    fn trailing_zeros(&self) -> usize {
        (*self as u64).trailing_zeros() as usize
    }

    fn trailing_ones(&self) -> usize {
        (*self as u64).trailing_ones() as usize
    }
}
impl UnsignedNumeric for u128 {
    const BITS: usize = 128;
    const ZERO: Self = 0u128;
    const ONE: Self = 1u128;
    const MAX: Self = u128::MAX;

    fn trailing_zeros(&self) -> usize {
        (*self as u128).trailing_zeros() as usize
    }

    fn trailing_ones(&self) -> usize {
        (*self as u128).trailing_ones() as usize
    }
}
