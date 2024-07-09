use core::num::TryFromIntError;

use num_enum::TryFromPrimitiveError;
use trie::TrieIndexError;

use crate::raw::{syscall, CapError, CapId, RawOperation, SyscallArgs};

pub mod cap_table;
pub mod hardware;
pub mod ipc;
pub mod thread;

pub enum InvalidOperation {
    BadOp,
    InvalidArgument,
}

pub trait SyscallOp: Sized + Copy {
    type R;

    /// Performs the syscall associated with this operation
    ///
    /// # Safety
    ///
    /// Syscalls can fundamentally change memory
    unsafe fn syscall(self, capability: CapId) -> Result<Self::R, CapError> {
        let args = self.make_args();
        unsafe { syscall(capability, args).map(|code| self.convert_success_code(code)) }
    }

    fn make_args(&self) -> SyscallArgs<'_>;
    fn from_args(args: SyscallArgs) -> Result<Self, InvalidOperation>;
    fn convert_success_code(&self, code: usize) -> Self::R;
}

impl From<TrieIndexError> for InvalidOperation {
    fn from(_value: TrieIndexError) -> Self {
        Self::InvalidArgument
    }
}

impl From<TryFromPrimitiveError<RawOperation>> for InvalidOperation {
    fn from(_value: TryFromPrimitiveError<RawOperation>) -> Self {
        Self::InvalidArgument
    }
}

impl From<TryFromIntError> for InvalidOperation {
    fn from(_value: TryFromIntError) -> Self {
        Self::InvalidArgument
    }
}

impl From<InvalidOperation> for CapError {
    fn from(value: InvalidOperation) -> Self {
        match value {
            InvalidOperation::BadOp => Self::InvalidOp,
            InvalidOperation::InvalidArgument => Self::InvalidArgument,
        }
    }
}
