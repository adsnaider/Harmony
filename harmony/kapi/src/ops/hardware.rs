use super::{InvalidOperation, SyscallOp};
use crate::raw::{RawOperation, SyscallArgs};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HardwareOp {
    EnableIoPorts,
    FlushPage { addr: usize },
}

impl SyscallOp for HardwareOp {
    type R = ();

    fn make_args(&self) -> SyscallArgs<'_> {
        match self {
            HardwareOp::EnableIoPorts => {
                SyscallArgs::new(RawOperation::HardwareAccessEnable.into(), 0, 0, 0, 0)
            }
            &HardwareOp::FlushPage { addr } => {
                SyscallArgs::new(RawOperation::HardwareFlushPage.into(), addr, 0, 0, 0)
            }
        }
    }

    fn from_args(args: SyscallArgs) -> Result<Self, InvalidOperation> {
        let op = RawOperation::try_from(args.op())?;
        match op {
            RawOperation::HardwareAccessEnable => Ok(Self::EnableIoPorts),
            RawOperation::HardwareFlushPage => Ok(Self::FlushPage {
                addr: args.args().0,
            }),
            _ => Err(InvalidOperation::BadOp),
        }
    }

    fn convert_success_code(&self, _code: usize) -> Self::R {}
}
