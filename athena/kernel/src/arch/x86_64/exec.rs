//! x86-64 execution context.

use core::arch::asm;

use super::paging::RawFrame;

/// Execution context that can be dispatched.
#[repr(C)]
pub struct ExecCtx {
    regs: Regs,
    l4_frame: RawFrame,
}

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct Regs {
    // General purpose
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rsi: u64,
    pub rbp: u64,

    // Control registers
    pub rsp: u64,
    pub rip: u64,
    // First arg
    pub rdi: u64,
    // Specialized registers??
    pub rflags: u64,
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
                "mov cr3, rbx",            // Current CR3
                "mov [rdi + 8 * 17], rax", // New cr3
                "cmp rax, rbx",
                "je 2f",
                "mov cr3, rax",
                "2:",
                // Restore the registers
                "mov rax, [rdi + 8*0]",
                "mov rbx, [rdi + 8*1]",
                "mov rcx, [rdi + 8*2]",
                "mov rdx, [rdi + 8*3]",
                "mov r8, [rdi + 8*4]",
                "mov r9, [rdi + 8*5]",
                "mov r10, [rdi + 8*6]",
                "mov r11, [rdi + 8*7]",
                "mov r12, [rdi + 8*8]",
                "mov r13, [rdi + 8*9]",
                "mov r14, [rdi + 8*10]",
                "mov r15, [rdi + 8*11]",
                "mov rsi, [rdi + 8*12]",
                "mov rbp, [rdi + 8*13]",
                "push (4 * 8) | 3",      // SS
                "push [rdi + 8*14]",     // Push rsp
                "push [rdi + 8*17]",     // push rflags
                "push (3 * 8) | 3",      // CS with RPL 3
                "push [rdi + 8*15]",     // Push the new instruction pointer
                "mov rdi, [rdi + 8*16]", // And the RDI register
                "iretq",
                options(noreturn)
            )
        }
    }
}
