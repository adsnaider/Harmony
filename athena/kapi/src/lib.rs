//! Kernel <--> Userspace API
#![no_std]
#![feature(naked_functions)]

use core::arch::asm;

#[naked]
pub unsafe extern "sysv64" fn raw_syscall(
    cap: usize,
    op: usize,
    a: usize,
    b: usize,
    c: usize,
    d: usize,
) -> isize {
    // NOTE: We don't need to align the stack on an int instruction.
    asm!("int 0x80", "ret", options(noreturn));
}

pub unsafe fn syscall(cap: CapId, op: Operation, args: SyscallArgs) -> Result<usize, CapError> {
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

use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(usize)]
pub enum Operation {
    ThdActivate = 0,

    CapLink = 1,
    CapUnlink = 2,
    CapConstruct = 3,
    CapRemove = 4,

    PageTableMap = 5,
    PageTableUnmap = 6,
    PageTableLink = 7,
    PageTableUnlink = 8,
    PageTableRetype = 9,
}

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CapError {
    ResourceInUse = 1,
    NotFound = 2,
    InvalidOp = 3,
    InvalidOpForResource = 4,
    InvalidArgument = 5,
}

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ResourceType {
    CapabilityTable = 0,
    ThreadControlBlock = 1,
    PageTable = 2,
}
impl From<<Operation as TryFrom<usize>>::Error> for CapError {
    fn from(_value: <Operation as TryFrom<usize>>::Error) -> Self {
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
