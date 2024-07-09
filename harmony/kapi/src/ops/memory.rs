use super::{InvalidOperation, SyscallOp};
use crate::raw::{RawOperation, SyscallArgs};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RetypeOp {
    pub region: usize,
    pub to: RetypeKind,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RetypeKind {
    Retype2Kernel,
    Retype2User,
    Retype2Untyped,
}

impl SyscallOp for RetypeOp {
    type R = ();

    fn make_args(&self) -> SyscallArgs<'_> {
        match self.to {
            RetypeKind::Retype2Kernel => {
                SyscallArgs::new(RawOperation::Retype2Kernel.into(), self.region, 0, 0, 0)
            }
            RetypeKind::Retype2User => {
                SyscallArgs::new(RawOperation::Retype2User.into(), self.region, 0, 0, 0)
            }
            RetypeKind::Retype2Untyped => {
                SyscallArgs::new(RawOperation::Retype2Untyped.into(), self.region, 0, 0, 0)
            }
        }
    }

    fn from_args(args: SyscallArgs) -> Result<Self, InvalidOperation> {
        let op = args.op().try_into()?;
        match op {
            RawOperation::Retype2Kernel => Ok(Self {
                region: args.args().0,
                to: RetypeKind::Retype2Kernel,
            }),
            RawOperation::Retype2User => Ok(Self {
                region: args.args().0,
                to: RetypeKind::Retype2User,
            }),
            RawOperation::Retype2Untyped => Ok(Self {
                region: args.args().0,
                to: RetypeKind::Retype2Untyped,
            }),
            _ => Err(InvalidOperation::BadOp),
        }
    }

    fn convert_success_code(&self, _code: usize) -> Self::R {}
}
