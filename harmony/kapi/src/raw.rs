use core::arch::asm;
use core::marker::PhantomData;
use core::num::TryFromIntError;

use bytemuck::{AnyBitPattern, NoUninit};
use num_enum::{IntoPrimitive, TryFromPrimitive, TryFromPrimitiveError};

use crate::ops::SlotId;

/// Performs a raw syscall
///
/// # Safety
///
/// Performing a syscall is inherently unsafe, follow the syscall
/// documentation to guarantee proper usage and soundness.
#[naked]
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
    CapTableLink,
    CapTableUnlink,
    CapTableConstruct,
    CapTableDrop,
    CapTableCopy,
    PageTableLink,
    PageTableUnlink,
    PageTableMapFrame,
    PageTableUnmapFrame,
    MemoryRegionRetype,
    MemoryRegionSplit,
    HardwareAccessEnable,
    HardwareFlushPage,
    SyncCall,
    SyncRet,
    Retype2Kernel,
    Retype2User,
    Retype2Untyped,
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
    InvalidFrame,
    MissingRightsToFrame,
    BadFrameType,
    SyncCallLimit,
    SyncRetBottom,
    FrameInUse,
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
pub struct SyscallArgs<'a> {
    op: usize,
    general: (usize, usize, usize, usize),
    _life: PhantomData<&'a ()>,
}

impl SyscallArgs<'_> {
    pub fn new(op: usize, a: usize, b: usize, c: usize, d: usize) -> Self {
        Self {
            op,
            general: (a, b, c, d),
            _life: PhantomData,
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
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, AnyBitPattern, NoUninit)]
pub struct CapId(u32);

impl CapId {
    pub const NUM_OFFSETS: usize = 32usize.div_ceil(SlotId::bits());
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn get(&self) -> u32 {
        self.0
    }

    pub const fn offsets(&self) -> [u32; Self::NUM_OFFSETS] {
        let mut cap = self.get();
        let mut out = [0; CapId::NUM_OFFSETS];
        let mut i = 0;
        while cap > 0 {
            const MASK: u32 = SlotId::count() as u32 - 1;
            let offset = cap & MASK;
            out[i] = offset;
            cap >>= SlotId::bits();
            i += 1;
        }
        out
    }

    pub fn from_offsets(offsets: [u32; Self::NUM_OFFSETS]) -> Self {
        let mut cap = 0;
        let trailing_zeros = offsets
            .iter()
            .position(|&off| off != 0)
            .unwrap_or(offsets.len());
        let valid_offsets = offsets.len() - trailing_zeros;
        for off in offsets.iter().copied().rev().take(valid_offsets) {
            cap <<= SlotId::bits();
            cap |= off;
        }
        Self::new(cap)
    }
}

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
