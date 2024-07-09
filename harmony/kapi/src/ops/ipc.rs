//! Operation for the synchronous invocation call gate.

use core::convert::Infallible;

use super::{InvalidOperation, SyscallOp};
use crate::raw::{RawOperation, SyscallArgs};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SyncCallOp {
    Call((usize, usize, usize, usize)),
}

impl SyscallOp for SyncCallOp {
    type R = usize;

    fn make_args(&self) -> crate::raw::SyscallArgs<'_> {
        match self {
            SyncCallOp::Call((a, b, c, d)) => {
                SyscallArgs::new(RawOperation::SyncCall.into(), *a, *b, *c, *d)
            }
        }
    }

    fn from_args(args: crate::raw::SyscallArgs) -> Result<Self, super::InvalidOperation> {
        let op = args.op().try_into()?;
        match op {
            RawOperation::SyncCall => Ok(SyncCallOp::Call(args.args())),
            _ => Err(InvalidOperation::BadOp),
        }
    }

    fn convert_success_code(&self, code: usize) -> Self::R {
        code
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SyncRetOp {
    SyncRet(usize),
}

impl SyscallOp for SyncRetOp {
    type R = Infallible;

    fn make_args(&self) -> SyscallArgs<'_> {
        match *self {
            SyncRetOp::SyncRet(return_code) => {
                SyscallArgs::new(RawOperation::SyncRet.into(), return_code, 0, 0, 0)
            }
        }
    }

    fn from_args(args: SyscallArgs) -> Result<Self, InvalidOperation> {
        let op: RawOperation = args.op().try_into()?;
        match op {
            RawOperation::SyncRet => Ok(Self::SyncRet(args.args().0)),
            _ => Err(InvalidOperation::BadOp),
        }
    }

    fn convert_success_code(&self, _code: usize) -> Self::R {
        unreachable!()
    }
}
