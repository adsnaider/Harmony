use core::slice;

use bytemuck::{AnyBitPattern, NoUninit};
pub use trie::SlotId;

use super::{InvalidOperation, SyscallOp};
use crate::raw::{CapId, RawOperation, SyscallArgs};

#[derive(Debug, Copy, Clone)]
#[repr(usize)]
pub enum ConstructArgs {
    CapTable = 0,
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
}

#[repr(C)]
#[derive(Debug, Copy, Clone, AnyBitPattern, NoUninit)]
pub struct PageTableConsArgs {
    pub level: u8,
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
pub struct ConsArgs<const SLOT_COUNT: usize> {
    pub kind: ConstructArgs,
    pub region: usize,
    pub slot: SlotId<SLOT_COUNT>,
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
    Construct(ConsArgs<SLOT_COUNT>),
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
                    ConstructArgs::CapTable => (0, [].as_slice()),
                    ConstructArgs::Thread(ref thd_args) => (1, bytemuck::bytes_of(thd_args)),
                    ConstructArgs::PageTable(ref pgt_args) => (2, bytemuck::bytes_of(pgt_args)),
                    ConstructArgs::SyncCall(ref sync_call_args) => {
                        (3, bytemuck::bytes_of(sync_call_args))
                    }
                };
                SyscallArgs::new(
                    RawOperation::CapTableConstruct as usize,
                    cons_args.region,
                    cons_args.slot.into(),
                    var,
                    args.as_ptr() as usize,
                )
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
                let (region, slot, kind, data) = args.args();

                let data = data as *const _;
                let kind = unsafe {
                    match kind {
                        0 => ConstructArgs::CapTable,
                        1 => ConstructArgs::Thread(*bytemuck::from_bytes(slice::from_raw_parts(
                            data,
                            core::mem::size_of::<ThreadConsArgs>(),
                        ))),
                        2 => ConstructArgs::PageTable(*bytemuck::from_bytes(
                            slice::from_raw_parts(data, core::mem::size_of::<PageTableConsArgs>()),
                        )),
                        _ => return Err(InvalidOperation::InvalidArgument),
                    }
                };
                Ok(Self::Construct(ConsArgs {
                    kind,
                    region,
                    slot: slot.try_into()?,
                }))
            }
            RawOperation::CapTableDrop => todo!(),
            RawOperation::CapTableCopy => todo!(),
            _ => Err(InvalidOperation::BadOp),
        }
    }

    fn convert_success_code(&self, _code: usize) -> Self::R {}
}
