//! A collection of resources provided to userspace threads.

use core::cell::{RefCell, UnsafeCell};

use heapless::Vec;
use kapi::ops::cap_table::{
    CapTableOp, ConsArgs, ConstructArgs, PageTableConsArgs, SyncCallConsArgs, ThreadConsArgs,
};
use kapi::ops::hardware::HardwareOp;
use kapi::ops::ipc::{SyncCallOp, SyncRetOp};
use kapi::ops::thread::ThreadOp;
use kapi::ops::SyscallOp;
use kapi::raw::{CapError, CapId, SyscallArgs};
use sync::cell::AtomicOnceCell;

use crate::arch::exec::{ControlRegs, ExecCtx, NoopSaver, Regs, SaveState, ScratchRegs};
use crate::arch::interrupts::SyscallCtx;
use crate::arch::paging::page_table::{Addrspace, AnyPageTable, PageTableFlags};
use crate::arch::paging::{Page, RawFrame, VirtAddr};
use crate::caps::{CapEntryExtension as _, PageCapFlags, RawCapEntry, Resource};
use crate::core_local::CoreLocal;
use crate::kptr::KPtr;
use crate::UNTYPED_MEMORY_OFFSET;

static ACTIVE_THREAD: AtomicOnceCell<CoreLocal<RefCell<Option<KPtr<Thread>>>>> =
    AtomicOnceCell::new();

pub fn init() {
    let threads = CoreLocal::new_with(|_| RefCell::new(None));
    ACTIVE_THREAD.set(threads).unwrap();
}

// TODO: Implement thread migration
/// A user-space thread that provides a mechanism for dispatching.
///
/// Each thread has its own address space, execution context, and resource
/// table.
#[repr(align(4096))]
pub struct Thread {
    // FIXME: This is not the correct way to do this...
    exec_ctx: UnsafeCell<ExecCtx>,
    root_comp: Component,
    component_stack: UnsafeCell<Vec<(Component, SyscallCtx), 16>>,
}

#[derive(Debug, Clone)]
pub struct Component {
    resources: KPtr<RawCapEntry>,
    page_table: KPtr<AnyPageTable>,
}

impl Thread {
    pub fn new(regs: Regs, component: Component) -> Self {
        let exec_ctx = ExecCtx::new(regs);
        Self {
            exec_ctx: UnsafeCell::new(exec_ctx),
            root_comp: component,
            component_stack: UnsafeCell::new(Vec::new()),
        }
    }

    pub fn component(&self) -> &Component {
        unsafe {
            (*self.component_stack.get())
                .last()
                .map(|(comp, _)| comp)
                .unwrap_or(&self.root_comp)
        }
    }

    pub fn current() -> Option<KPtr<Thread>> {
        ACTIVE_THREAD.get().unwrap().get().borrow().clone()
    }

    pub fn dispatch(this: KPtr<Self>, saver: impl SaveState) -> ! {
        // Our kernel is non-preemptive which makes every other case really
        // simple as it's a completely synchronous call-response. However, thread
        // dispatching is somewhat weird because we exit the kernel early on the
        // dispatch and never return back to the caller in a traditional sense (i.e.
        // dispatch return !). The way we come back is by having another dispatch
        // call back into the original thread. Note, we have a singular kernel
        // execution stack, so once we leave here, the stack will be mangled and
        // can't come back to the kernel to return to the normal flow of execution.
        //
        // When that happens, the state of the (current) thread needs to be valid,
        // specifically, to the thread it needs to look like the original Activate
        // call returned with a success status code. So here's what needs to happen
        //
        // 1. Return register needs to be 0.
        // 2. rflags register needs to be valid (interrupts enabled, ring 3 execution, etc.)
        // 3. stack register needs to be whatever it was before syscall
        // 4. All callee-saved registers need to be set back (done in userspace)
        {
            let mut current = ACTIVE_THREAD.get().unwrap().get().borrow_mut();
            if let Some(ref current) = *current {
                let regs = unsafe { (*current.exec_ctx.get()).regs_mut() };
                saver.save_state(regs);
            }
            current.replace(this.clone());
        }
        log::info!("Set the active thread");
        unsafe {
            this.component().page_table.as_addrspace().make_active();
            (*this.exec_ctx.get()).dispatch();
        }
    }

    pub fn exercise_cap(
        self: KPtr<Self>,
        capability: CapId,
        args: SyscallArgs,
    ) -> Result<usize, CapError> {
        log::debug!("Syscall for: {capability:?}, {args:?}");
        let slot = self.component().resources.clone().find(capability)?.get();
        match slot.resource {
            Resource::Empty => Err(CapError::NotFound),
            Resource::CapEntry(capability_table) => {
                let operation = CapTableOp::from_args(args)?;
                match operation {
                    CapTableOp::Link {
                        other_table_cap,
                        slot,
                    } => {
                        let other_table: KPtr<RawCapEntry> = self
                            .component()
                            .resources
                            .clone()
                            .get_resource_as(other_table_cap)?;
                        let slot = capability_table.index_slot(slot);
                        slot.change(|cap| {
                            cap.child = Some(other_table);
                        });
                        Ok(0)
                    }
                    CapTableOp::Unlink { slot } => {
                        let slot = capability_table.index_slot(slot);
                        slot.change(|cap| {
                            cap.child = None;
                        });
                        Ok(0)
                    }
                    CapTableOp::Construct(ConsArgs { kind, region, slot }) => {
                        if region > RawFrame::memory_limit() {
                            return Err(CapError::InvalidFrame);
                        }
                        let page_address = region + UNTYPED_MEMORY_OFFSET;
                        let region = Page::try_from_start_address(
                            VirtAddr::try_new(page_address)
                                .map_err(|_| CapError::InvalidArgument)?,
                        )
                        .map_err(|_| CapError::InvalidArgument)?;

                        let (frame, flags) = self
                            .component()
                            .addrspace()
                            .get(region)
                            .ok_or(CapError::Internal)?;
                        if !flags.contains(PageTableFlags::PRESENT) {
                            return Err(CapError::MissingRightsToFrame);
                        }
                        let resource = match kind {
                            ConstructArgs::CapTable => {
                                let ptr = KPtr::new(frame, RawCapEntry::default())
                                    .map_err(|_| CapError::BadFrameType)?;
                                Resource::CapEntry(ptr)
                            }
                            ConstructArgs::Thread(ThreadConsArgs {
                                entry,
                                stack_pointer,
                                cap_table,
                                page_table,
                                arg0,
                            }) => {
                                let regs = Regs {
                                    control: ControlRegs {
                                        rip: entry as u64,
                                        rsp: stack_pointer as u64,
                                        rflags: 0x202,
                                    },
                                    scratch: ScratchRegs {
                                        rdi: arg0 as u64,
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                };
                                let cap_table: KPtr<RawCapEntry> = self
                                    .component()
                                    .resources
                                    .clone()
                                    .get_resource_as(cap_table)?;
                                let (page_table, flags): (KPtr<AnyPageTable>, PageCapFlags) = self
                                    .component()
                                    .resources
                                    .clone()
                                    .get_resource_as(page_table)?;
                                if !flags.level() == 4 {
                                    return Err(CapError::InvalidArgument);
                                }
                                Resource::Thread(
                                    KPtr::new(
                                        frame,
                                        Thread::new(regs, Component::new(cap_table, page_table)),
                                    )
                                    .map_err(|_| CapError::BadFrameType)?,
                                )
                            }
                            ConstructArgs::PageTable(PageTableConsArgs { level }) => {
                                if level > 4 || level == 0 {
                                    return Err(CapError::InvalidArgument);
                                }
                                let table = if level == 4 {
                                    AnyPageTable::clone_kernel()
                                } else {
                                    AnyPageTable::new()
                                };
                                let flags = PageCapFlags::new(level);
                                Resource::PageTable {
                                    table: KPtr::new(frame, table)
                                        .map_err(|_| CapError::BadFrameType)?,
                                    flags,
                                }
                            }
                            ConstructArgs::SyncCall(SyncCallConsArgs {
                                entry,
                                cap_table,
                                page_table,
                            }) => {
                                let cap_table: KPtr<RawCapEntry> = self
                                    .component()
                                    .resources
                                    .clone()
                                    .get_resource_as(cap_table)?;
                                let (page_table, _): (KPtr<AnyPageTable>, PageCapFlags) = self
                                    .component()
                                    .resources
                                    .clone()
                                    .get_resource_as(page_table)?;
                                let component = Component::new(cap_table, page_table);
                                Resource::SyncCall { entry, component }
                            }
                        };
                        capability_table.index_slot(slot).change(|cap| {
                            cap.resource = resource;
                        });
                        Ok(0)
                    }
                    CapTableOp::Drop { slot: _ } => todo!(),
                    CapTableOp::Copy {
                        slot: _,
                        other_table_cap: _,
                        other_slot: _,
                    } => todo!(),
                }
            }
            Resource::Thread(thread) => {
                let operation = ThreadOp::from_args(args)?;
                match operation {
                    ThreadOp::Activate => {
                        // SAFETY: Running a syscall.
                        let ctx = unsafe { SyscallCtx::current() };
                        Thread::dispatch(thread, ctx);
                    }
                    ThreadOp::ChangeAffinity => todo!(),
                }
            }
            Resource::PageTable { table: _, flags: _ } => todo!(),
            Resource::HardwareAccess => {
                let op = HardwareOp::from_args(args)?;
                match op {
                    HardwareOp::EnableIoPorts => {
                        const IOPL3: u64 = 0x3000;
                        unsafe {
                            let flags = SyscallCtx::get_flags();
                            SyscallCtx::update_flags(flags | IOPL3);
                        }
                        Ok(0)
                    }
                }
            }
            Resource::SyncCall { entry, component } => {
                let op = SyncCallOp::from_args(args)?;
                match op {
                    SyncCallOp::Call((rdi, rsi, rdx, rcx)) => {
                        let regs = unsafe {
                            (*self.component_stack.get())
                                .push((component, SyscallCtx::current()))
                                .map_err(|_| CapError::SyncCallLimit)?;
                            (*self.exec_ctx.get()).regs_mut()
                        };
                        regs.scratch.rdi = rdi as u64;
                        regs.scratch.rsi = rsi as u64;
                        regs.scratch.rdx = rdx as u64;
                        regs.scratch.rcx = rcx as u64;
                        regs.control.rip = entry as u64;
                        regs.control.rsp = 0;
                        regs.control.rflags = 0x202;
                        Self::dispatch(self, NoopSaver::new())
                    }
                }
            }
            Resource::SyncRet => {
                let op = SyncRetOp::from_args(args)?;
                match op {
                    SyncRetOp::SyncRet(code) => unsafe {
                        let (_comp, syscall_ctx) = (*self.component_stack.get())
                            .pop()
                            .ok_or(CapError::SyncRetBottom)?;
                        // Set the return codes (rax for the syscall itself and rdi for the return of the invocation)
                        (*self.exec_ctx.get()).regs_mut().scratch.rax = u64::max(0, code as u64);
                        (*self.exec_ctx.get()).regs_mut().preserved = syscall_ctx.preserved_regs;
                        (*self.exec_ctx.get()).regs_mut().control = syscall_ctx.control_regs;

                        Self::dispatch(self, NoopSaver::new())
                    },
                }
            }
        }
    }
}

impl Component {
    pub fn new(resources: KPtr<RawCapEntry>, page_table: KPtr<AnyPageTable>) -> Self {
        Self {
            resources,
            page_table,
        }
    }

    pub fn addrspace(&self) -> Addrspace<'_> {
        unsafe { self.page_table.as_addrspace() }
    }
}
