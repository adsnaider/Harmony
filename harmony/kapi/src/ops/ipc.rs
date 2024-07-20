//! Operation for the synchronous invocation call gate.

use core::arch::asm;
use core::convert::Infallible;

use super::{InvalidOperation, SyscallOp};
use crate::raw::{CapError, RawOperation, SyscallArgs};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SyncCallOp {
    Call((usize, usize, usize, usize)),
}

impl SyscallOp for SyncCallOp {
    type R = isize;

    unsafe fn syscall(
        self,
        capability: crate::raw::CapId,
    ) -> Result<Self::R, crate::raw::CapError> {
        let args = self.make_args();
        let op = args.op();
        let (a, b, c, d) = args.args();
        let kernel_code: isize;
        let call_code: isize;

        unsafe {
            asm!("int 0x80",
                inlateout("rdi") capability.get() => _,
                inlateout("rsi") op => _,
                inlateout("rdx") a => call_code,
                inlateout("rcx") b => _,
                inlateout("r8") c => _,
                inlateout("r9") d => _,
                lateout("rax") kernel_code,
                lateout("r10") _,
                lateout("r11") _,
                options(preserves_flags, nostack)
            );
        }
        usize::try_from(kernel_code)
            .map_err(|_| CapError::try_from((-kernel_code) as u8).unwrap())?;
        Ok(call_code)
    }

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

    fn convert_success_code(&self, _code: usize) -> Self::R {
        unimplemented!("SyncCall has 2 codes, the kernel and the result. Don't use this directly");
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
