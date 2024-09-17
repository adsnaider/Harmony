use core::slice;

use bytemuck::{AnyBitPattern, NoUninit};

use super::{InvalidOperation, SlotId, SyscallOp};
use crate::raw::{CapId, RawOperation, SyscallArgs};

#[derive(Debug, Copy, Clone)]
#[repr(usize)]
pub enum ConstructArgs {
    CapTable(CapTableConsArgs),
    Thread(ThreadConsArgs),
    PageTable(PageTableConsArgs),
    SyncCall(SyncCallConsArgs),
}

#[repr(C)]
#[derive(Debug, Copy, Clone, AnyBitPattern, NoUninit)]
pub struct ThreadConsArgs {
    pub entry: usize,
    pub stack_pointer: usize,
    pub cap_table: CapId,
    pub page_table: CapId,
    pub arg0: usize,
    pub region: usize,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, AnyBitPattern, NoUninit)]
pub struct PageTableConsArgs {
    pub region: usize,
    pub level: u8,
    pub _padding: [u8; 7],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, AnyBitPattern, NoUninit)]
pub struct CapTableConsArgs {
    pub region: usize,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, AnyBitPattern, NoUninit)]
pub struct SyncCallConsArgs {
    pub entry: usize,
    pub cap_table: CapId,
    pub page_table: CapId,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ConsArgs {
    pub kind: ConstructArgs,
    pub slot: SlotId,
}

#[derive(Debug, Copy, Clone)]
pub enum CapTableOp {
    Link {
        slot: SlotId,
        other_table_cap: CapId,
    },
    Unlink {
        slot: SlotId,
    },
    Construct(ConsArgs),
    Drop {
        slot: SlotId,
    },
    Copy {
        slot: SlotId,
        other_table_cap: CapId,
        other_slot: SlotId,
    },
}

impl SyscallOp for CapTableOp {
    type R = ();

    fn make_args(&self) -> SyscallArgs {
        match *self {
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
            CapTableOp::Construct(ref cons_args) => {
                let (var, args) = match cons_args.kind {
                    ConstructArgs::CapTable(ref cap_args) => (0, bytemuck::bytes_of(cap_args)),
                    ConstructArgs::Thread(ref thd_args) => (1, bytemuck::bytes_of(thd_args)),
                    ConstructArgs::PageTable(ref pgt_args) => (2, bytemuck::bytes_of(pgt_args)),
                    ConstructArgs::SyncCall(ref sync_call_args) => {
                        (3, bytemuck::bytes_of(sync_call_args))
                    }
                };
                SyscallArgs::new(
                    RawOperation::CapTableConstruct as usize,
                    cons_args.slot.into(),
                    var,
                    args.as_ptr() as usize,
                    0,
                )
            }
            CapTableOp::Drop { slot: _ } => todo!(),
            CapTableOp::Copy {
                slot,
                other_table_cap,
                other_slot,
            } => SyscallArgs::new(
                RawOperation::CapTableCopy.into(),
                slot.into(),
                other_table_cap.into(),
                other_slot.into(),
                0,
            ),
        }
    }

    fn from_args(args: SyscallArgs) -> Result<Self, InvalidOperation> {
        let op = RawOperation::try_from(args.op())?;
        match op {
            RawOperation::CapTableLink => {
                let other_table_cap = CapId::try_from(args.args().0)?;
                let slot = args.args().1.try_into()?;
                Ok(Self::Link {
                    other_table_cap,
                    slot,
                })
            }
            RawOperation::CapTableUnlink => {
                let slot = args.args().0.try_into()?;
                Ok(Self::Unlink { slot })
            }
            RawOperation::CapTableConstruct => {
                let (slot, kind, data, _) = args.args();

                let data = data as *const _;
                let kind = unsafe {
                    match kind {
                        0 => ConstructArgs::CapTable(*bytemuck::from_bytes(slice::from_raw_parts(
                            data,
                            core::mem::size_of::<CapTableConsArgs>(),
                        ))),
                        1 => ConstructArgs::Thread(*bytemuck::from_bytes(slice::from_raw_parts(
                            data,
                            core::mem::size_of::<ThreadConsArgs>(),
                        ))),
                        2 => ConstructArgs::PageTable(*bytemuck::from_bytes(
                            slice::from_raw_parts(data, core::mem::size_of::<PageTableConsArgs>()),
                        )),
                        3 => ConstructArgs::SyncCall(*bytemuck::from_bytes(slice::from_raw_parts(
                            data,
                            core::mem::size_of::<SyncCallConsArgs>(),
                        ))),
                        _ => return Err(InvalidOperation::InvalidArgument),
                    }
                };
                Ok(Self::Construct(ConsArgs {
                    kind,
                    slot: slot.try_into()?,
                }))
            }
            RawOperation::CapTableDrop => todo!(),
            RawOperation::CapTableCopy => {
                let (slot, other_table_cap, other_slot, ..) = args.args();
                Ok(Self::Copy {
                    slot: slot.try_into()?,
                    other_table_cap: other_table_cap.try_into()?,
                    other_slot: other_slot.try_into()?,
                })
            }
            _ => Err(InvalidOperation::BadOp),
        }
    }

    fn convert_success_code(&self, _code: usize) -> Self::R {}
}
