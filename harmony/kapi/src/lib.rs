//! Kernel <--> Userspace API
#![no_std]
#![feature(naked_functions)]

use core::arch::asm;

#[naked]
/// Performs a raw syscall
///
/// # Safety
///
/// Performing a syscall is inherently unsafe, follow the syscall
/// documentation to guarantee proper usage and soundness.
pub unsafe extern "sysv64" fn raw_syscall(
    _a: usize,
    _b: usize,
    _c: usize,
    _d: usize,
    _e: usize,
    _f: usize,
) -> isize {
    // NOTE: We don't need to align the stack on an int instruction.
    asm!("int 0x80", "ret", options(noreturn));
}

/// Performs a syscall
///
/// # Safety
///
/// Performing a syscall is inherently unsafe, follow the syscall
/// documentation to guarantee proper usage and soundness.
pub unsafe fn syscall(
    cap: CapId,
    op: impl Into<usize>,
    args: SyscallArgs,
) -> Result<usize, CapError> {
    let result = unsafe {
        raw_syscall(
            u32::from(cap).try_into().unwrap(),
            op.into(),
            args.0,
            args.1,
            args.2,
            args.3,
        )
    };
    match usize::try_from(result) {
        Ok(ret) => Ok(ret),
        Err(_) => Err(CapError::try_from((-result) as u8).unwrap()),
    }
}

use num_enum::{IntoPrimitive, TryFromPrimitive, TryFromPrimitiveError};

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(usize)]
pub enum ThreadOp {
    Activate = 0,
    ChangeAffinity = 1,
}

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(usize)]
pub enum CapTableOp {
    Link = 2,
    Unlink,
    Construct,
    Drop,
    Copy,
}

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(usize)]
pub enum PageTableOp {
    Link = 7,
    Unlink,
}

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(usize)]
pub enum MemoryRegionOp {
    Retype = 9,
    Split,
}

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CapError {
    ResourceInUse = 1,
    NotFound,
    InvalidOp,
    InvalidOpForResource,
    InvalidArgument,
    PageOffsetOutOfBounds,
    FrameOutsideOfRegion,
    FrameNotUser,
}

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ResourceType {
    CapabilityTable = 0,
    ThreadControlBlock,
    PageTable,
}

impl<T: TryFromPrimitive> From<TryFromPrimitiveError<T>> for CapError {
    fn from(_value: TryFromPrimitiveError<T>) -> Self {
        Self::InvalidOp
    }
}

impl CapError {
    pub fn to_errno(self) -> isize {
        let errno: isize = (self as u8).into();
        errno
    }
}

#[cfg(feature = "from_errors")]
mod convert_errors {
    use sync::cell::BorrowError;
    use trie::TrieIndexError;

    use super::*;

    impl From<BorrowError> for CapError {
        fn from(value: BorrowError) -> Self {
            match value {
                BorrowError::AlreadyBorrowed => CapError::ResourceInUse,
            }
        }
    }

    impl From<TrieIndexError> for CapError {
        fn from(value: TrieIndexError) -> Self {
            match value {
                TrieIndexError::OutOfBounds => CapError::InvalidArgument,
            }
        }
    }
}

#[repr(C)]
pub struct SyscallArgs(usize, usize, usize, usize);

impl SyscallArgs {
    pub fn new(a: usize, b: usize, c: usize, d: usize) -> Self {
        (a, b, c, d).into()
    }

    pub fn to_tuple(self) -> (usize, usize, usize, usize) {
        self.into()
    }
}

impl From<(usize, usize, usize, usize)> for SyscallArgs {
    fn from(value: (usize, usize, usize, usize)) -> Self {
        Self(value.0, value.1, value.2, value.3)
    }
}

impl From<SyscallArgs> for (usize, usize, usize, usize) {
    fn from(value: SyscallArgs) -> Self {
        (value.0, value.1, value.2, value.3)
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct CapId(u32);

impl From<u32> for CapId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<CapId> for u32 {
    fn from(value: CapId) -> Self {
        value.0
    }
}

#[repr(usize)]
#[derive(Debug, Copy, Clone, IntoPrimitive, TryFromPrimitive)]
pub enum FrameType {
    Untyped = 0,
    User = 1,
    Kernel = 2,
}
