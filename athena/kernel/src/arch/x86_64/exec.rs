//! x86-64 execution context.

use super::paging::RawFrame;

/// Execution context that can be dispatched.
#[repr(C)]
pub struct ExecCtx {
    l4_frame: RawFrame,
    regs: Regs,
}

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct Regs {
    // Control registers
    pub rip: u64,
    pub rsp: u64,
    // General purpose
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rdi: u64,
    pub rsi: u64,
    // Specialized registers??
}

impl ExecCtx {
    pub fn new(l4_frame: RawFrame, regs: Regs) -> Self {
        Self { l4_frame, regs }
    }

    pub fn regs(&self) -> &Regs {
        &self.regs
    }

    pub fn regs_mut(&mut self) -> &mut Regs {
        &mut self.regs
    }

    pub fn l4_frame(&self) -> RawFrame {
        self.l4_frame
    }

    pub fn set_l4_frame(&mut self, l4_frame: RawFrame) {
        self.l4_frame = l4_frame;
    }

    pub fn dispatch(&self) -> ! {
        todo!();
    }
}
