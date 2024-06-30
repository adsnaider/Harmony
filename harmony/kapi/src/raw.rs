use core::arch::asm;
use core::num::TryFromIntError;

use num_enum::{IntoPrimitive, TryFromPrimitive, TryFromPrimitiveError};

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
pub unsafe fn syscall(cap: CapId, args: SyscallArgs) -> Result<usize, CapError> {
    let result = unsafe {
        raw_syscall(
            u32::from(cap).try_into().unwrap(),
            args.op(),
            args.args().0,
            args.args().1,
            args.args().2,
            args.args().3,
        )
    };
    match usize::try_from(result) {
        Ok(ret) => Ok(ret),
        Err(_) => Err(CapError::try_from((-result) as u8).unwrap()),
    }
}

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(usize)]
pub enum RawOperation {
    ThreadActivate = 0,
    ThreadChangeAffinity,
    CapTableLink,
    CapTableUnlink,
    CapTableConstruct,
    CapTableDrop,
    CapTableCopy,
    PageTableLink,
    PageTableUnlink,
    MemoryRegionRetype,
    MemoryRegionSplit,
}

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CapError {
    ResourceInUse = 1,
    NotFound,
    InvalidOp,
    InvalidArgument,
    PageOffsetOutOfBounds,
    FrameOutsideOfRegion,
    FrameNotUser,
    Internal,
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
        -errno
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
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SyscallArgs {
    op: usize,
    general: (usize, usize, usize, usize),
}

impl SyscallArgs {
    pub fn new(op: usize, a: usize, b: usize, c: usize, d: usize) -> Self {
        Self {
            op,
            general: (a, b, c, d),
        }
    }

    pub fn op(&self) -> usize {
        self.op
    }

    pub fn args(&self) -> (usize, usize, usize, usize) {
        self.general
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct CapId(u32);

pub struct OutOfBounds;

impl From<u32> for CapId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl TryFrom<usize> for CapId {
    type Error = TryFromIntError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Ok(Self(u32::try_from(value)?))
    }
}

impl From<CapId> for u32 {
    fn from(value: CapId) -> Self {
        value.0
    }
}

impl From<CapId> for usize {
    fn from(value: CapId) -> Self {
        value.0.try_into().unwrap()
    }
}

#[repr(usize)]
#[derive(Debug, Copy, Clone, IntoPrimitive, TryFromPrimitive)]
pub enum FrameType {
    Untyped = 0,
    User = 1,
    Kernel = 2,
}
