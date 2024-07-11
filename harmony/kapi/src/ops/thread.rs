use super::{InvalidOperation, SyscallOp};
use crate::raw::{RawOperation, SyscallArgs};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ThreadOp {
    Activate,
}

impl SyscallOp for ThreadOp {
    type R = ();

    fn make_args(&self) -> SyscallArgs {
        match self {
            ThreadOp::Activate => SyscallArgs::new(RawOperation::ThreadActivate.into(), 0, 0, 0, 0),
        }
    }

    fn from_args(args: SyscallArgs) -> Result<Self, InvalidOperation> {
        let op = RawOperation::try_from(args.op())?;
        match op {
            RawOperation::ThreadActivate => Ok(Self::Activate),
            _ => Err(InvalidOperation::BadOp),
        }
    }

    fn convert_success_code(&self, _code: usize) -> Self::R {}
}
