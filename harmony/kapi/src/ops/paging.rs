use bitflags::bitflags;

use super::{InvalidOperation, SyscallOp};
use crate::raw::{CapId, RawOperation, SyscallArgs};

bitflags! {
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
    pub struct PermissionMask: usize {
        const WRITE = 0x0001;
        const EXECUTE = 0x0002;
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PageTableOp {
    Link {
        other_table: CapId,
        slot: usize,
        permissions: PermissionMask,
    },
    Unlink {
        slot: usize,
    },
    MapFrame {
        user_frame: usize,
        slot: usize,
        permissions: PermissionMask,
    },
    UnmapFrame {
        slot: usize,
    },
}

impl SyscallOp for PageTableOp {
    type R = ();

    fn make_args(&self) -> SyscallArgs<'_> {
        match *self {
            PageTableOp::Link {
                other_table,
                slot,
                permissions,
            } => SyscallArgs::new(
                RawOperation::PageTableLink.into(),
                other_table.into(),
                slot,
                permissions.bits(),
                0,
            ),
            PageTableOp::Unlink { slot } => {
                SyscallArgs::new(RawOperation::PageTableUnlink.into(), slot, 0, 0, 0)
            }
            PageTableOp::MapFrame {
                user_frame,
                slot,
                permissions,
            } => SyscallArgs::new(
                RawOperation::PageTableMapFrame.into(),
                user_frame,
                slot,
                permissions.bits(),
                0,
            ),
            PageTableOp::UnmapFrame { slot } => {
                SyscallArgs::new(RawOperation::PageTableUnmapFrame.into(), slot, 0, 0, 0)
            }
        }
    }

    fn from_args(args: SyscallArgs) -> Result<Self, super::InvalidOperation> {
        let op = RawOperation::try_from(args.op())?;
        match op {
            RawOperation::PageTableLink => Ok(Self::Link {
                other_table: CapId::try_from(args.args().0)?,
                slot: args.args().1,
                permissions: PermissionMask::from_bits_truncate(args.args().2),
            }),
            RawOperation::PageTableUnlink => Ok(Self::Unlink {
                slot: args.args().0,
            }),
            RawOperation::PageTableMapFrame => Ok(Self::MapFrame {
                user_frame: args.args().0,
                slot: args.args().1,
                permissions: PermissionMask::from_bits_truncate(args.args().2),
            }),
            RawOperation::PageTableUnmapFrame => Ok(Self::UnmapFrame {
                slot: args.args().0,
            }),
            _ => Err(InvalidOperation::BadOp),
        }
    }

    fn convert_success_code(&self, _code: usize) -> Self::R {}
}
