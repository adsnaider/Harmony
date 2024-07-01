//! x86-64 execution context.

use core::arch::asm;

use super::paging::RawFrame;

pub trait SaveState: Sized {
    fn save_state(self, regs: &mut Regs);
}

/// Execution context that can be dispatched.
#[repr(C)]
pub struct ExecCtx {
    regs: Regs,
    l4_frame: RawFrame, // Off: 18
}

// SAFETY: Don't change the order of any of these
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct PreservedRegs {
    pub rbx: u64,
    pub rbp: u64, // Off: 10
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

// SAFETY: Don't change the order of any of these
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct ScratchRegs {
    pub rax: u64, // Off: 0
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64, // Off: 5
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
}

// SAFETY: Don't change the order of any of these
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct ControlRegs {
    pub rflags: u64, // Off: 15
    pub rsp: u64,
    pub rip: u64,
}

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct Regs {
    pub scratch: ScratchRegs,
    pub preserved: PreservedRegs,
    pub control: ControlRegs,
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

    #[naked]
    pub extern "sysv64" fn dispatch(&self) -> ! {
        // SAFETY: All ExecCtx must be safe to dispatch. Every l4_frame
        // must have the top half kernel mapped.
        unsafe {
            asm!(
                "pop rax",
                "mov rbx, cr3",            // Current CR3
                "mov rax, [rdi + 8 * 18]", // New cr3
                "cmp rax, rbx",
                "mov cr3, rax",
                // Setup the segment selectors
                "mov ax, (4 * 8) | 3",
                "mov ds, ax",
                "mov es, ax",
                "mov fs, ax",
                "mov gs, ax",
                // Restore SCRATCH
                "mov rax, [rdi + 8*0]",
                "mov rcx, [rdi + 8*1]",
                "mov rdx, [rdi + 8*2]",
                "mov rsi, [rdi + 8*3]",
                // RDI: Later as it holds arg0
                "mov r8, [rdi + 8*5]",
                "mov r9, [rdi + 8*6]",
                "mov r10, [rdi + 8*7]",
                "mov r11, [rdi + 8*8]",
                // Restore PRESEVED
                "mov rbx, [rdi + 8*9]",
                "mov rbp, [rdi + 8*10]",
                "mov r12, [rdi + 8*11]",
                "mov r13, [rdi + 8*12]",
                "mov r14, [rdi + 8*13]",
                "mov r15, [rdi + 8*14]",
                "push (4 * 8) | 3",     // SS
                "push [rdi + 8*16]",    // Push rsp
                "push [rdi + 8*15]",    // push rflags
                "push (3 * 8) | 3",     // CS with RPL 3
                "push [rdi + 8*17]",    // Push the new instruction pointer
                "mov rdi, [rdi + 8*4]", // And the RDI register
                "iretq",
                options(noreturn)
            )
        }
    }
}
