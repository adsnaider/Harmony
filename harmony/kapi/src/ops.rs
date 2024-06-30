use crate::raw::{syscall, CapError, CapId, SyscallArgs};

pub enum InvalidOperation {
    BadOp,
    InvalidArgument,
}

pub trait SyscallOp: Sized + Copy {
    type R;

    unsafe fn syscall(self, capability: CapId) -> Result<Self::R, CapError> {
        unsafe { syscall(capability, self.into_args()).map(|code| self.from_success_code(code)) }
    }

    fn into_args(self) -> SyscallArgs;
    fn from_args(args: SyscallArgs) -> Result<Self, InvalidOperation>;
    fn from_success_code(&self, code: usize) -> Self::R;
}

pub mod thread {}

pub mod cap_table {
    use trie::SlotId;

    use super::{InvalidOperation, SyscallOp};
    use crate::raw::{CapId, RawOperation, SyscallArgs};

    #[derive(Debug, Copy, Clone)]
    #[repr(C)]
    pub enum ConstructArgs {
        CapTable,
        Thread {
            entry: usize,
            stack_pointer: usize,
            cap_table: CapId,
            page_table: CapId,
        },
        PageTable {
            level: u8,
        },
    }

    #[derive(Debug, Copy, Clone)]
    pub enum CapTableOp<const SLOT_COUNT: usize> {
        Link {
            slot: SlotId<SLOT_COUNT>,
            other_table_cap: CapId,
        },
        Unlink {
            slot: SlotId<SLOT_COUNT>,
        },
        Construct {
            kind: ConstructArgs,
            region: usize,
            slot: SlotId<SLOT_COUNT>,
        },
        Drop {
            slot: SlotId<SLOT_COUNT>,
        },
        Copy {
            slot: SlotId<SLOT_COUNT>,
            other_table_cap: CapId,
            other_slot: SlotId<SLOT_COUNT>,
        },
    }

    impl<const SLOT_COUNT: usize> SyscallOp for CapTableOp<SLOT_COUNT> {
        type R = ();

        fn into_args(self) -> SyscallArgs {
            match self {
                CapTableOp::Link {
                    other_table_cap,
                    slot,
                } => SyscallArgs::new(
                    RawOperation::CapTableLink.into(),
                    other_table_cap.into(),
                    slot.into(),
                    0,
                    0,
                ),
                CapTableOp::Unlink { slot } => {
                    SyscallArgs::new(RawOperation::CapTableUnlink.into(), slot.into(), 0, 0, 0)
                }
                CapTableOp::Construct {
                    kind: _,
                    slot: _,
                    region: _,
                } => {
                    todo!()
                }
                CapTableOp::Drop { slot: _ } => todo!(),
                CapTableOp::Copy {
                    slot: _,
                    other_table_cap: _,
                    other_slot: _,
                } => todo!(),
            }
        }

        fn from_args(args: SyscallArgs) -> Result<Self, InvalidOperation> {
            let op = RawOperation::try_from(args.op()).map_err(|_| InvalidOperation::BadOp)?;
            match op {
                RawOperation::CapTableLink => {
                    let other_table_cap = CapId::try_from(args.args().0)
                        .map_err(|_| InvalidOperation::InvalidArgument)?;
                    let slot = args
                        .args()
                        .1
                        .try_into()
                        .map_err(|_| InvalidOperation::InvalidArgument)?;
                    Ok(Self::Link {
                        other_table_cap,
                        slot,
                    })
                }
                RawOperation::CapTableUnlink => {
                    let slot = args
                        .args()
                        .0
                        .try_into()
                        .map_err(|_| InvalidOperation::InvalidArgument)?;
                    Ok(Self::Unlink { slot })
                }
                RawOperation::CapTableConstruct => todo!(),
                RawOperation::CapTableDrop => todo!(),
                RawOperation::CapTableCopy => todo!(),
                _ => Err(InvalidOperation::BadOp),
            }
        }

        fn from_success_code(&self, _code: usize) -> Self::R {
            ()
        }
    }
}
